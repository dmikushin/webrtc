#ifndef WEBRTC_C_API_H
#define WEBRTC_C_API_H

#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif

typedef struct webrtc_session webrtc_session_t;
typedef void (*webrtc_input_callback_t)(const void* data, int len, void* user_data);

webrtc_session_t* webrtc_session_create(const char* config_json, webrtc_input_callback_t cb, void* user_data);
void webrtc_session_send_frame(webrtc_session_t* session, int width, int height, const uint8_t* yuv);
void webrtc_session_destroy(webrtc_session_t* session);

#ifdef __cplusplus
}
#endif

#endif // WEBRTC_C_API_H
