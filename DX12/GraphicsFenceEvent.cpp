#include "GraphicsFenceEvent.h"

KAIROS_EXPORT_BEGIN 

int GraphicsFenceEvent_Create(GraphicsFenceEvent* _this)
{
    _this->m_Handle = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    if (_this->m_Handle == nullptr)
        return CreateFenceEventFailed;

    return GraphicsSuccess;
}

void KAIROS_API GraphicsFenceEvent_Dispose(GraphicsFenceEvent* _this)
{
    CloseHandle(_this->m_Handle);
}

DWORD KAIROS_API GraphicsFenceEvent_Wait(GraphicsFenceEvent* _this, DWORD dwMilliseconds)
{
    return WaitForSingleObject(_this->m_Handle, dwMilliseconds);
}



KAIROS_EXPORT_END