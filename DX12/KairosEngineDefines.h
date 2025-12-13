#ifndef __KAIROS_ENGINE_DEFINES__
#define __KAIROS_ENGINE_DEFINES__

//#define KAIROS_EXPORT_LIB

#ifdef KAIROS_EXPORT_LIB
#define KAIROS_EXPORT_BEGIN extern "C" {
#define KAIROS_EXPORT_END }
#define KAIROS_API __cdecl
#else
#define KAIROS_EXPORT_BEGIN
#define KAIROS_EXPORT_END
#define KAIROS_API
#endif

#endif

