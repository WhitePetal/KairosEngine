#ifndef __KAIROS_GRAPHICS_DEVICE__
#define __KAIROS_GRAPHICS_DEVICE__

#include "KairosEngineDefines.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

class GraphicsDevice
{
public:
	int Initialize();
	void DeInitialize();

private:
	ID3D12Device* m_pDevice;
};

KAIROS_EXPORT_BEGIN

GraphicsDevice* KAIROS_API GraphicsDevice_Create();

int KAIROS_API GraphicsDevice_Initialize(GraphicsDevice* _this);

void KAIROS_API Graphics_DeInitialize(GraphicsDevice* _this);

KAIROS_EXPORT_END

#endif
