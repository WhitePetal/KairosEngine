#ifndef __KAIROS_KERNEL__
#define __KAIROS_KERNEL__

#include "KairosEngineDefines.h"
#include "Windows.h"

KAIROS_EXPORT_BEGIN

bool KAIROS_API KairosQueryPerformanceFrequency(INT64* perf_frequency);

bool KAIROS_API KairosQueryPerformanceCounter(INT64* pref_counter);

int KAIROS_API KairosGetKeyboardCodePgaeI(UINT* pKeyboardCodePage, int codePageSize);

bool KAIROS_API KairosGetMonitorInfo(HMONITOR monitor, MONITORINFO* pInfo);

KAIROS_EXPORT_END
#endif