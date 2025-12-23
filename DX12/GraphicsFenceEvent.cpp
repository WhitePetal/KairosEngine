#include "GraphicsFenceEvent.h"

int GraphicsFenceEvent::Create()
{
    m_Handle = CreateEventW(nullptr, FALSE, FALSE, nullptr);
    if (m_Handle == nullptr)
        return CreateFenceEventFailed;

    return GraphicsSuccess;
}

void GraphicsFenceEvent::Dispose()
{
    CloseHandle(m_Handle);
}


KAIROS_EXPORT_BEGIN


KAIROS_EXPORT_END

KAIROS_EXPORT_BEGIN CreateResult GraphicsFenceEvent_Create()
{
    GraphicsFenceEvent* _this = new GraphicsFenceEvent{};
    int hr = _this->Create();
    return CreateResult{ hr, _this };
}

void KAIROS_API GraphicsFenceEvent_Dispose(GraphicsFenceEvent* _this)
{
    _this->Dispose();
    delete _this;
}



KAIROS_EXPORT_END