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



KAIROS_EXPORT_END