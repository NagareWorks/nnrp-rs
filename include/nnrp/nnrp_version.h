#ifndef NNRP_VERSION_H
#define NNRP_VERSION_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define NNRP_SDK_VERSION "1.0.0-preview.4.4"
#define NNRP_SDK_VERSION_MAJOR 1
#define NNRP_SDK_VERSION_MINOR 0
#define NNRP_SDK_VERSION_PATCH 0
#define NNRP_SDK_VERSION_PREVIEW 4
#define NNRP_SDK_VERSION_REVISION 2

#define NNRP_PROTOCOL_MAJOR 1
#define NNRP_PROTOCOL_WIRE_FORMAT 0

typedef struct NnrpSdkVersion {
  uint16_t major;
  uint16_t minor;
  uint16_t patch;
  uint16_t preview;
  uint16_t revision;
} NnrpSdkVersion;

#ifdef __cplusplus
}
#endif

#endif
