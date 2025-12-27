#include "FenceEvent.h"

KAIROS_EXPORT_BEGIN 

int FenceEvent_Create(HANDLE* p_this)
{
    HANDLE handle = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    if (handle == nullptr)
        return 1;
    *p_this = handle;
    return 0;
}

void KAIROS_API FenceEvent_Close(HANDLE _this)
{
    CloseHandle(_this);
}

DWORD KAIROS_API FenceEvent_Wait(HANDLE _this, DWORD dwMilliseconds)
{
    return WaitForSingleObject(_this, dwMilliseconds);
}



KAIROS_EXPORT_END