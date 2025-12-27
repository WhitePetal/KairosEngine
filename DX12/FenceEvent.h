#ifndef __KAIROS_GRAPHICS_FENCE_EVENT__
#define __KAIROS_GRAPHICS_FENCE_EVENT__

#include "KairosEngineDefines.h"
#include <d3d12.h>


KAIROS_EXPORT_BEGIN

int FenceEvent_Create(HANDLE* p_this);

void KAIROS_API FenceEvent_Close(HANDLE _this);

DWORD KAIROS_API FenceEvent_Wait(HANDLE _this, DWORD dwMilliseconds);

KAIROS_EXPORT_END


#endif
