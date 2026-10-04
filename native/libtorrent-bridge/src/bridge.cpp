#include "media_torrent_bridge.h"

#include <libtorrent/hex.hpp>
#include <libtorrent/load_torrent.hpp>
#include <libtorrent/magnet_uri.hpp>
#include <libtorrent/read_resume_data.hpp>
#include <libtorrent/session.hpp>
#include <libtorrent/settings_pack.hpp>
#include <libtorrent/torrent_handle.hpp>
#include <libtorrent/torrent_status.hpp>
#include <libtorrent/write_resume_data.hpp>

#include <algorithm>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <stdexcept>
#include <string>
#include <vector>

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <Windows.h>
#endif

namespace fs = std::filesystem;
namespace lt = libtorrent;

struct mt_engine {
    lt::session session;
    fs::path state_directory;
    std::string download_directory;

    mt_engine(fs::path state, std::string download)
        : session(lt::settings_pack{}), state_directory(std::move(state)),
          download_directory(std::move(download)) {}
};

namespace {

void copy_text(char* destination, int32_t capacity, std::string const& text) {
    if (destination == nullptr || capacity <= 0) return;
    auto const count = std::min<std::size_t>(text.size(), static_cast<std::size_t>(capacity - 1));
    std::memcpy(destination, text.data(), count);
    destination[count] = '\0';
}

template <std::size_t N>
void copy_text(char (&destination)[N], std::string const& text) {
    copy_text(destination, static_cast<int32_t>(N), text);
}

template <typename Function>
int32_t guarded(Function&& function, char* error, int32_t error_capacity) {
    try {
        return function();
    } catch (std::exception const& exception) {
        copy_text(error, error_capacity, exception.what());
        return -1;
    } catch (...) {
        copy_text(error, error_capacity, "Unknown libtorrent error.");
        return -1;
    }
}

mt_engine& require_engine(mt_engine* engine) {
    if (engine == nullptr) throw std::invalid_argument("Torrent engine is not initialized.");
    return *engine;
}

std::string hash_key(lt::torrent_handle const& handle) {
    return lt::aux::to_hex(handle.info_hashes().get_best().to_string());
}

lt::torrent_handle find_torrent(mt_engine& engine, char const* info_hash) {
    if (info_hash == nullptr || *info_hash == '\0') throw std::invalid_argument("Missing torrent info hash.");
    auto const handles = engine.session.get_torrents();
    auto const found = std::find_if(handles.begin(), handles.end(), [info_hash](auto const& handle) {
        return hash_key(handle) == info_hash;
    });
    if (found == handles.end()) throw std::invalid_argument("Torrent not found.");
    return *found;
}

std::vector<char> read_file(fs::path const& path) {
    std::ifstream input(path, std::ios::binary);
    if (!input) throw std::runtime_error("Cannot open torrent state file.");
    return {std::istreambuf_iterator<char>(input), std::istreambuf_iterator<char>()};
}

void write_file_atomically(fs::path const& destination, std::vector<char> const& bytes) {
    fs::path temporary = destination;
    temporary += L".tmp";
    {
        std::ofstream output(temporary, std::ios::binary | std::ios::trunc);
        if (!output) throw std::runtime_error("Cannot write torrent state file.");
        output.write(bytes.data(), static_cast<std::streamsize>(bytes.size()));
        output.flush();
        if (!output) throw std::runtime_error("Cannot finish writing torrent state file.");
    }
#ifdef _WIN32
    if (!MoveFileExW(temporary.c_str(), destination.c_str(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH))
        throw std::runtime_error("Cannot replace torrent state file.");
#else
    fs::rename(temporary, destination);
#endif
}

void save_all(mt_engine& engine) {
    for (auto const& handle : engine.session.get_torrents()) {
        auto const bytes = lt::write_resume_data_buf(handle.get_resume_data());
        write_file_atomically(engine.state_directory / (hash_key(handle) + ".resume"), bytes);
    }
}

void restore_all(mt_engine& engine) {
    for (auto const& entry : fs::directory_iterator(engine.state_directory)) {
        if (!entry.is_regular_file() || entry.path().extension() != ".resume") continue;
        try {
            auto params = lt::read_resume_data(read_file(entry.path()));
            engine.session.add_torrent(std::move(params));
        } catch (...) {
            // A damaged resume file must not prevent the rest of the queue from loading.
        }
    }
}

int32_t add_torrent(mt_engine& engine, lt::add_torrent_params params,
    char* info_hash, int32_t hash_capacity) {
    if (params.save_path.empty()) params.save_path = engine.download_directory;
    auto const handle = engine.session.add_torrent(std::move(params));
    copy_text(info_hash, hash_capacity, hash_key(handle));
    save_all(engine);
    return 0;
}

} // namespace

extern "C" {

mt_engine* mt_create(char const* state_directory, char const* download_directory,
    char* error, int32_t error_capacity) {
    try {
        if (!state_directory || !*state_directory || !download_directory || !*download_directory)
            throw std::invalid_argument("Torrent state and download directories are required.");
        auto const state = fs::u8path(state_directory);
        fs::create_directories(state);
        fs::create_directories(fs::u8path(download_directory));
        auto* engine = new mt_engine(state, download_directory);
        restore_all(*engine);
        return engine;
    } catch (std::exception const& exception) {
        copy_text(error, error_capacity, exception.what());
        return nullptr;
    } catch (...) {
        copy_text(error, error_capacity, "Could not start libtorrent.");
        return nullptr;
    }
}

void mt_destroy(mt_engine* engine) {
    if (!engine) return;
    try { save_all(*engine); } catch (...) { /* explicit mt_save reports failures */ }
    delete engine;
}

int32_t mt_add_to(mt_engine* engine, char const* value, int32_t is_file, char const* destination,
    char* info_hash, int32_t hash_capacity, char* error, int32_t error_capacity) {
    return guarded([&] {
        if (!destination || !*destination || !fs::is_directory(fs::u8path(destination)))
            throw std::invalid_argument("A valid download folder is required.");
        if (!value || !*value || (!is_file && std::strncmp(value, "magnet:?", 8) != 0))
            throw std::invalid_argument("A magnet link or .torrent file is required.");
        auto params = is_file ? lt::load_torrent_buffer(read_file(fs::u8path(value))) : lt::parse_magnet_uri(value);
        params.save_path = destination;
        return add_torrent(require_engine(engine), std::move(params), info_hash, hash_capacity);
    }, error, error_capacity);
}

int32_t mt_save_path(mt_engine* engine, char const* info_hash, char* path, int32_t capacity,
    char* error, int32_t error_capacity) {
    return guarded([&] {
        auto const handle = find_torrent(require_engine(engine), info_hash);
        copy_text(path, capacity, handle.status().save_path);
        return 0;
    }, error, error_capacity);
}

int32_t mt_add_magnet(mt_engine* engine, char const* uri,
    char* info_hash, int32_t hash_capacity, char* error, int32_t error_capacity) {
    return guarded([&] {
        if (!uri || std::strncmp(uri, "magnet:?", 8) != 0)
            throw std::invalid_argument("A magnet link is required.");
        return add_torrent(require_engine(engine), lt::parse_magnet_uri(uri), info_hash, hash_capacity);
    }, error, error_capacity);
}

int32_t mt_add_torrent_file(mt_engine* engine, char const* path,
    char* info_hash, int32_t hash_capacity, char* error, int32_t error_capacity) {
    return guarded([&] {
        if (!path || !*path) throw std::invalid_argument("A .torrent path is required.");
        auto const bytes = read_file(fs::u8path(path));
        return add_torrent(require_engine(engine), lt::load_torrent_buffer(bytes), info_hash, hash_capacity);
    }, error, error_capacity);
}

int32_t mt_list(mt_engine* engine, mt_transfer* out, int32_t capacity,
    char* error, int32_t error_capacity) {
    return guarded([&] {
        if (capacity < 0 || (capacity > 0 && out == nullptr))
            throw std::invalid_argument("Invalid transfer buffer.");
        auto const handles = require_engine(engine).session.get_torrents();
        auto const count = std::min<std::size_t>(handles.size(), static_cast<std::size_t>(capacity));
        for (std::size_t index = 0; index < count; ++index) {
            auto const& handle = handles[index];
            auto const status = handle.status();
            auto& transfer = out[index];
            std::memset(&transfer, 0, sizeof(transfer));
            copy_text(transfer.info_hash, hash_key(handle));
            copy_text(transfer.name, status.name);
            if (status.errc) copy_text(transfer.error, status.errc.message());
            transfer.state = static_cast<int32_t>(status.state);
            transfer.paused = static_cast<bool>(handle.flags() & lt::torrent_flags::paused) ? 1 : 0;
            transfer.auto_managed = static_cast<bool>(handle.flags() & lt::torrent_flags::auto_managed) ? 1 : 0;
            transfer.queue_position = static_cast<int32_t>(status.queue_position);
            transfer.peers = status.num_peers;
            transfer.seeds = status.num_seeds;
            transfer.progress = status.progress;
            transfer.size_bytes = status.total_wanted;
            transfer.downloaded_bytes = status.total_wanted_done;
            transfer.uploaded_bytes = status.all_time_upload;
            transfer.download_rate = status.download_rate;
            transfer.upload_rate = status.upload_rate;
        }
        return static_cast<int32_t>(handles.size());
    }, error, error_capacity);
}

int32_t mt_files(mt_engine* engine, char const* info_hash, mt_file* out,
    int32_t capacity, char* error, int32_t error_capacity) {
    return guarded([&] {
        if (capacity < 0 || (capacity > 0 && !out)) throw std::invalid_argument("Invalid file buffer.");
        auto const handle = find_torrent(require_engine(engine), info_hash);
        auto const info = handle.torrent_file();
        if (!info) return int32_t(0);
        auto const& files = info->layout();
        auto const renames = handle.get_renamed_files();
        lt::filenames const names(files, renames);
        auto const progress = handle.file_progress();
        auto const save_path = fs::u8path(handle.status().save_path);
        int32_t count = 0;
        for (auto const index : files.file_range()) {
            if (files.pad_file_at(index)) continue;
            if (count < capacity) {
                auto const path = (save_path / fs::u8path(names.file_path(index))).u8string();
                if (path.size() >= sizeof(out[count].path)) throw std::runtime_error("Torrent file path is too long.");
                std::memset(&out[count], 0, sizeof(mt_file));
                copy_text(out[count].path, path);
                out[count].size_bytes = files.file_size(index);
                out[count].completed_bytes = progress[static_cast<std::size_t>(static_cast<int>(index))];
            }
            ++count;
        }
        return count;
    }, error, error_capacity);
}

int32_t mt_set_paused(mt_engine* engine, char const* info_hash, int32_t paused,
    char* error, int32_t error_capacity) {
    return guarded([&] {
        auto const handle = find_torrent(require_engine(engine), info_hash);
        if (paused) {
            handle.unset_flags(lt::torrent_flags::auto_managed);
            handle.pause();
        } else {
            handle.set_flags(lt::torrent_flags::auto_managed);
            handle.resume();
        }
        return 0;
    }, error, error_capacity);
}

int32_t mt_move_queue(mt_engine* engine, char const* info_hash, int32_t direction,
    char* error, int32_t error_capacity) {
    return guarded([&] {
        auto const handle = find_torrent(require_engine(engine), info_hash);
        if (direction < 0) handle.queue_position_up();
        else if (direction > 0) handle.queue_position_down();
        else throw std::invalid_argument("Queue direction must be -1 or 1.");
        return 0;
    }, error, error_capacity);
}

int32_t mt_set_limits(mt_engine* engine, int32_t download_bytes_per_second,
    int32_t upload_bytes_per_second, char* error, int32_t error_capacity) {
    return guarded([&] {
        if (download_bytes_per_second < 0 || upload_bytes_per_second < 0)
            throw std::invalid_argument("Speed limits cannot be negative.");
        lt::settings_pack settings;
        settings.set_int(lt::settings_pack::download_rate_limit, download_bytes_per_second);
        settings.set_int(lt::settings_pack::upload_rate_limit, upload_bytes_per_second);
        require_engine(engine).session.apply_settings(settings);
        return 0;
    }, error, error_capacity);
}

int32_t mt_remove(mt_engine* engine, char const* info_hash,
    char* error, int32_t error_capacity) {
    return guarded([&] {
        auto& state = require_engine(engine);
        auto const handle = find_torrent(state, info_hash);
        auto const key = hash_key(handle);
        state.session.remove_torrent(handle); // keep the downloaded files
        fs::remove(state.state_directory / (key + ".resume"));
        return 0;
    }, error, error_capacity);
}

int32_t mt_save(mt_engine* engine, char* error, int32_t error_capacity) {
    return guarded([&] { save_all(require_engine(engine)); return 0; }, error, error_capacity);
}

} // extern "C"
