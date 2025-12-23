#ifndef __KAIROS_GRAPHICS_FENCE_EVENT__
#define __KAIROS_GRAPHICS_FENCE_EVENT__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include "CreateResult.h"
#include <d3d12.h>

class GraphicsFenceEvent
{
public:
	int Create();

	void Dispose();

private:
	HANDLE m_Handle;
};

KAIROS_EXPORT_BEGIN

CreateResult GraphicsFenceEvent_Create();

void KAIROS_API GraphicsFenceEvent_Dispose(GraphicsFenceEvent* _this);

KAIROS_EXPORT_END


#endif
