#ifndef __KAIROS_GRAPHICS_COMMAND_LIST__
#define __KAIROS_GRAPHICS_COMMAND_LIST__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include "CreateResult.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

class GraphicsCommandList
{
public:
	GraphicsCommandList(ID3D12CommandList* pCommandList);

	void Dispose();

private:
	ID3D12CommandList* m_pCommandList;
};

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandList_Dispose(GraphicsCommandList* _this);

KAIROS_EXPORT_END

#endif