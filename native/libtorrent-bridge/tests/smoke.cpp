#include "media_torrent_bridge.h"

#include <cstdio>

int main(int argc, char** argv) {
    if (argc != 3) return 2;
    char error[512]{};
    auto* engine = mt_create(argv[1], argv[2], error, sizeof(error));
    if (!engine) {
        std::fprintf(stderr, "create: %s\n", error);
        return 1;
    }
    auto const count = mt_list(engine, nullptr, 0, error, sizeof(error));
    if (count != 0) {
        std::fprintf(stderr, "expected empty queue, got %d (%s)\n", count, error);
        mt_destroy(engine);
        return 1;
    }
    if (mt_save(engine, error, sizeof(error)) != 0) {
        std::fprintf(stderr, "save: %s\n", error);
        mt_destroy(engine);
        return 1;
    }
    mt_destroy(engine);
    return 0;
}
