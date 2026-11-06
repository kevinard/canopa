/* Private restricted-FD consumer. Capture actual frame and SPA cursor metadata.
 * API: https://docs.pipewire.org/group__pw__stream.html */
#include <errno.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <pipewire/pipewire.h>
#include <spa/param/video/format-utils.h>
#include <spa/param/buffers.h>
#include <spa/pod/builder.h>

#define MAX_FRAME (64u * 1024u * 1024u)
#define MAX_CURSOR (sizeof(struct spa_meta_cursor) + sizeof(struct spa_meta_bitmap) + 384u * 384u * 4u)
struct capture {
    struct pw_main_loop *loop;
    struct pw_stream *stream;
    struct spa_video_info_raw format;
    const char *directory;
    int mode, result;
};

static int save_pixels(struct capture *capture, const char *name, const uint8_t *pixels,
                       uint32_t width, uint32_t height, uint32_t stride, bool alpha) {
    if (!width || !height || (uint64_t)width * height * 4 > MAX_FRAME || stride < (uint64_t)width * 4)
        return -1;
    char path[4096];
    if (snprintf(path, sizeof(path), "%s/%s", capture->directory, name) >= (int)sizeof(path)) return -1;
    FILE *file = fopen(path, "wb");
    if (!file) return -1;
    uint8_t *row = malloc((size_t)width * 4);
    if (!row) { fclose(file); return -1; }
    int result = 0;
    for (uint32_t y = 0; y < height; ++y) {
        const uint8_t *source = pixels + (size_t)y * stride;
        for (uint32_t x = 0; x < width; ++x) {
            row[x * 4] = source[x * 4 + 2];
            row[x * 4 + 1] = source[x * 4 + 1];
            row[x * 4 + 2] = source[x * 4];
            row[x * 4 + 3] = alpha ? source[x * 4 + 3] : 255;
        }
        if (fwrite(row, 4, width, file) != width) { result = -1; break; }
    }
    free(row);
    if (fclose(file)) result = -1;
    return result;
}

static void process(void *data) {
    struct capture *capture = data;
    struct pw_buffer *buffer = pw_stream_dequeue_buffer(capture->stream);
    if (!buffer) return;
    struct spa_buffer *spa = buffer->buffer;
    struct spa_meta *meta = spa_buffer_find_meta(spa, SPA_META_Cursor);
    struct spa_meta_cursor *cursor = meta && meta->size >= sizeof(*cursor) ? meta->data : NULL;
    bool valid = cursor && cursor->id;
    struct spa_meta_bitmap *bitmap = NULL;
    const uint8_t *bitmap_pixels = NULL;
    if (valid && cursor->bitmap_offset >= sizeof(*cursor) && cursor->bitmap_offset <= meta->size &&
        sizeof(struct spa_meta_bitmap) <= meta->size - cursor->bitmap_offset) {
        bitmap = (void *)((uint8_t *)cursor + cursor->bitmap_offset);
        size_t left = meta->size - cursor->bitmap_offset;
        if (bitmap->format == SPA_VIDEO_FORMAT_BGRA && bitmap->size.width && bitmap->size.height &&
            bitmap->size.width <= 384 && bitmap->size.height <= 384 &&
            bitmap->stride >= (int64_t)bitmap->size.width * 4 && bitmap->offset >= sizeof(*bitmap) &&
            bitmap->offset <= left && (uint64_t)bitmap->stride * bitmap->size.height <= left - bitmap->offset)
            bitmap_pixels = (uint8_t *)bitmap + bitmap->offset;
    }
    // Metadata mode must deliver a visible image, rather than only a position.
    if (capture->mode == 4 && (!bitmap_pixels || bitmap->size.width <= 1 || bitmap->size.height <= 1)) goto recycle;
    if (spa->n_datas != 1 || !spa->datas[0].data || !spa->datas[0].chunk) goto recycle;
    struct spa_data *plane = &spa->datas[0];
    struct spa_chunk *chunk = plane->chunk;
    uint32_t width = capture->format.size.width, height = capture->format.size.height;
    if (!width || !height || chunk->stride <= 0 || plane->maxsize > MAX_FRAME ||
        chunk->offset > plane->maxsize || chunk->size > plane->maxsize - chunk->offset ||
        (uint64_t)chunk->stride * height > chunk->size || (uint64_t)width * 4 > (uint32_t)chunk->stride) goto recycle;
    if (save_pixels(capture, "frame.rgba", (uint8_t *)plane->data + chunk->offset,
                    width, height, chunk->stride, capture->format.format == SPA_VIDEO_FORMAT_BGRA)) goto failed;
    if (bitmap_pixels && save_pixels(capture, "cursor.rgba", bitmap_pixels, bitmap->size.width,
                                     bitmap->size.height, bitmap->stride, true)) goto failed;
    struct spa_meta_header *header = spa_buffer_find_meta_data(spa, SPA_META_Header, sizeof(*header));
    printf("{\"width\":%u,\"height\":%u,\"sequence\":%"PRIu64",\"cursor_valid\":%s,"
           "\"position\":[%d,%d],\"hotspot\":[%d,%d],\"cursor_size\":[%u,%u]}\n",
           width, height, header ? header->seq : 0, valid ? "true" : "false",
           valid ? cursor->position.x : 0, valid ? cursor->position.y : 0,
           valid ? cursor->hotspot.x : 0, valid ? cursor->hotspot.y : 0,
           bitmap_pixels ? bitmap->size.width : 0, bitmap_pixels ? bitmap->size.height : 0);
    fflush(stdout);
    capture->result = 0;
    pw_stream_queue_buffer(capture->stream, buffer);
    pw_main_loop_quit(capture->loop);
    return;
failed:
    fprintf(stderr, "cannot save bounded capture buffers\n");
    pw_main_loop_quit(capture->loop);
recycle:
    pw_stream_queue_buffer(capture->stream, buffer);
}

static void parameters(void *data, uint32_t id, const struct spa_pod *param) {
    struct capture *capture = data;
    if (id != SPA_PARAM_Format || !param) return;
    if (spa_format_video_raw_parse(param, &capture->format) < 0 ||
        (capture->format.format != SPA_VIDEO_FORMAT_BGRx && capture->format.format != SPA_VIDEO_FORMAT_BGRA)) {
        pw_main_loop_quit(capture->loop); return;
    }
    uint8_t storage[1024];
    struct spa_pod_builder builder = SPA_POD_BUILDER_INIT(storage, sizeof(storage));
    const struct spa_pod *params[3];
    params[0] = spa_pod_builder_add_object(&builder, SPA_TYPE_OBJECT_ParamBuffers, SPA_PARAM_Buffers,
        SPA_PARAM_BUFFERS_dataType, SPA_POD_CHOICE_FLAGS_Int((1 << SPA_DATA_MemFd) | (1 << SPA_DATA_MemPtr)));
    params[1] = spa_pod_builder_add_object(&builder, SPA_TYPE_OBJECT_ParamMeta, SPA_PARAM_Meta,
        SPA_PARAM_META_type, SPA_POD_Id(SPA_META_Header), SPA_PARAM_META_size, SPA_POD_Int(sizeof(struct spa_meta_header)));
    params[2] = spa_pod_builder_add_object(&builder, SPA_TYPE_OBJECT_ParamMeta, SPA_PARAM_Meta,
        SPA_PARAM_META_type, SPA_POD_Id(SPA_META_Cursor), SPA_PARAM_META_size,
        SPA_POD_CHOICE_RANGE_Int(MAX_CURSOR, sizeof(struct spa_meta_cursor), MAX_CURSOR));
    if (pw_stream_update_params(capture->stream, params, 3) < 0) pw_main_loop_quit(capture->loop);
}
static void state(void *data, enum pw_stream_state old, enum pw_stream_state next, const char *error) {
    (void)old;
    if (next == PW_STREAM_STATE_ERROR) {
        struct capture *capture = data;
        fprintf(stderr, "stream failed: %s\n", error ? error : "unknown");
        pw_main_loop_quit(capture->loop);
    }
}
static const struct pw_stream_events events = {
    PW_VERSION_STREAM_EVENTS, .state_changed = state, .param_changed = parameters, .process = process
};

int main(int argc, char **argv) {
    if (argc != 5) return 2;
    char *end;
    errno = 0;
    long descriptor = strtol(argv[1], &end, 10);
    if (errno || *end || descriptor < 0 || descriptor > INT32_MAX) return 2;
    errno = 0;
    unsigned long long serial = strtoull(argv[2], &end, 10);
    if (errno || *end || !serial || argv[2][0] == '-') return 2;
    int mode = atoi(argv[4]);
    if (mode != 1 && mode != 2 && mode != 4) return 2;
    pw_init(NULL, NULL);
    struct capture capture = { .directory = argv[3], .mode = mode, .result = 1 };
    capture.loop = pw_main_loop_new(NULL);
    if (!capture.loop) return 3;
    struct pw_context *context = pw_context_new(pw_main_loop_get_loop(capture.loop), NULL, 0);
    if (!context) return 3;
    struct pw_core *core = pw_context_connect_fd(context, (int)descriptor, NULL, 0);
    if (!core) return 3;
    capture.stream = pw_stream_new(core, "Canopa private cursor consumer", pw_properties_new(
        PW_KEY_MEDIA_TYPE, "Video", PW_KEY_MEDIA_CATEGORY, "Capture", PW_KEY_MEDIA_ROLE, "Screen",
        PW_KEY_TARGET_OBJECT, argv[2], NULL));
    if (!capture.stream) return 3;
    struct spa_hook listener;
    pw_stream_add_listener(capture.stream, &listener, &events, &capture);
    uint8_t storage[1024];
    struct spa_pod_builder builder = SPA_POD_BUILDER_INIT(storage, sizeof(storage));
    const struct spa_pod *format = spa_pod_builder_add_object(&builder, SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat,
        SPA_FORMAT_mediaType, SPA_POD_Id(SPA_MEDIA_TYPE_video),
        SPA_FORMAT_mediaSubtype, SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw),
        SPA_FORMAT_VIDEO_format, SPA_POD_CHOICE_ENUM_Id(3, SPA_VIDEO_FORMAT_BGRx, SPA_VIDEO_FORMAT_BGRx, SPA_VIDEO_FORMAT_BGRA));
    if (pw_stream_connect(capture.stream, PW_DIRECTION_INPUT, PW_ID_ANY,
        PW_STREAM_FLAG_AUTOCONNECT | PW_STREAM_FLAG_MAP_BUFFERS, &format, 1) >= 0)
        pw_main_loop_run(capture.loop);
    pw_stream_destroy(capture.stream);
    pw_core_disconnect(core);
    pw_context_destroy(context);
    pw_main_loop_destroy(capture.loop);
    pw_deinit();
    return capture.result;
}
