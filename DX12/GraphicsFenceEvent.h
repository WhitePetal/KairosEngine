#ifndef __KAIROS_GRAPHICS_FENCE_EVENT__
#define __KAIROS_GRAPHICS_FENCE_EVENT__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include <d3d12.h>

typedef struct GraphicsFenceEvent
{
	HANDLE m_Handle;
} GraphicsFenceEvent;

KAIROS_EXPORT_BEGIN

int GraphicsFenceEvent_Create(GraphicsFenceEvent* _this);

void KAIROS_API GraphicsFenceEvent_Dispose(GraphicsFenceEvent* _this);

KAIROS_EXPORT_END


#endif
