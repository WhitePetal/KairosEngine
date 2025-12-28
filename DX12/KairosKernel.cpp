#include "KairosKernel.h"

KAIROS_EXPORT_BEGIN

bool KAIROS_API KairosQueryPerformanceFrequency(INT64* perf_frequency)
{
    return ::QueryPerformanceFrequency((LARGE_INTEGER*)perf_frequency);
}

bool KAIROS_API KairosQueryPerformanceCounter(INT64* pref_counter)
{
    return ::QueryPerformanceCounter((LARGE_INTEGER*)pref_counter);
}

int KAIROS_API KairosGetKeyboardCodePgaeI(UINT* pKeyboardCodePage, int codePageSize)
{
    HKL keyboard_layout = ::GetKeyboardLayout(0);
    LCID keyboard_lcid = MAKELCID(HIWORD(keyboard_layout), SORT_DEFAULT);
    return ::GetLocaleInfoA(keyboard_lcid, (LOCALE_RETURN_NUMBER | LOCALE_IDEFAULTANSICODEPAGE), (LPSTR)&pKeyboardCodePage, codePageSize);
}

bool KAIROS_API KairosGetMonitorInfo(HMONITOR monitor, MONITORINFO* pInfo)
{
    return ::GetMonitorInfo(monitor, pInfo);
}

KAIROS_EXPORT_END