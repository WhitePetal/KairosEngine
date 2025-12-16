#ifndef __KAIROS_GRAPHICS_DEVICE__
#define __KAIROS_GRAPHICS_DEVICE__

#include "KairosEngineDefines.h"
#include "GraphicsCommandQueue.h"
#include "CreateResult.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

class GraphicsDevice
{
public:
	int Create();
	void Dispose();

	CreateResult CreateaCommandQueue(D3D12_COMMAND_LIST_TYPE type, int priority, D3D12_COMMAND_QUEUE_FLAGS flags, UINT nodeMask);

private:
	ID3D12Device* m_pDevice;
};

KAIROS_EXPORT_BEGIN

GraphicsDevice* KAIROS_API GraphicsDevice_Allocate();

int KAIROS_API GraphicsDevice_Create(GraphicsDevice* _this);

void KAIROS_API GraphicsDevice_Dispose(GraphicsDevice* _this);

CreateResult KAIROS_API GraphicsDevice_CreateCommandQueue(GraphicsDevice* _this, D3D12_COMMAND_LIST_TYPE type, int priority, D3D12_COMMAND_QUEUE_FLAGS flags, int nodeMask);

KAIROS_EXPORT_END

#endif
