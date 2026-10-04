#pragma once

#include <stdint.h>

#ifdef _WIN32
#ifdef MT_BUILDING_LIBRARY
#define MT_API __declspec(dllexport)
#else
#define MT_API __declspec(dllimport)
#endif
#else
#define MT_API __attribute__((visibility("default")))
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef struct mt_engine mt_engine;

typedef struct mt_transfer {
    char info_hash[65];
    char name[512];
    char error[512];
    int32_t state;
    int32_t paused;
    int32_t auto_managed;
    int32_t queue_position;
    int32_t peers;
    int32_t seeds;
    double progress;
    int64_t size_bytes;
    int64_t downloaded_bytes;
    int64_t uploaded_bytes;
    int32_t download_rate;
    int32_t upload_rate;
} mt_transfer;

typedef struct mt_file {
    char path[4096];
    int64_t size_bytes;
    int64_t completed_bytes;
} mt_file;

// All strings are UTF-8. Error buffers may be null when capacity is zero.
// A caller must serialize access to one engine instance.
MT_API mt_engine* mt_create(char const* state_directory, char const* download_directory,
    char* error, int32_t error_capacity);
MT_API void mt_destroy(mt_engine* engine);
MT_API int32_t mt_add_magnet(mt_engine* engine, char const* uri,
    char* info_hash, int32_t hash_capacity, char* error, int32_t error_capacity);
MT_API int32_t mt_add_torrent_file(mt_engine* engine, char const* path,
    char* info_hash, int32_t hash_capacity, char* error, int32_t error_capacity);
// Returns the total transfer count. Copies up to capacity entries into out.
MT_API int32_t mt_list(mt_engine* engine, mt_transfer* out, int32_t capacity,
    char* error, int32_t error_capacity);
MT_API int32_t mt_files(mt_engine* engine, char const* info_hash, mt_file* out,
    int32_t capacity, char* error, int32_t error_capacity);
MT_API int32_t mt_set_paused(mt_engine* engine, char const* info_hash, int32_t paused,
    char* error, int32_t error_capacity);
MT_API int32_t mt_move_queue(mt_engine* engine, char const* info_hash, int32_t direction,
    char* error, int32_t error_capacity);
MT_API int32_t mt_set_limits(mt_engine* engine, int32_t download_bytes_per_second,
    int32_t upload_bytes_per_second, char* error, int32_t error_capacity);
// Removing a torrent never deletes its downloaded files.
MT_API int32_t mt_remove(mt_engine* engine, char const* info_hash,
    char* error, int32_t error_capacity);
MT_API int32_t mt_save(mt_engine* engine, char* error, int32_t error_capacity);

#ifdef __cplusplus
}
#endif
