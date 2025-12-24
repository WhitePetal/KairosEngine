#ifndef __KAIROS_GRAPHICS_BUFFER__
#define __KAIROS_GRAPHICS_BUFFER__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

typedef struct GraphicsBuffer
{
	ID3D12Resource* m_pBuffer;

} GraphicsBuffer;


KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsBuffer_Dispose(GraphicsBuffer* _this);

KAIROS_EXPORT_END

#endif