#ifndef __KAIROS_GRAPHICS_FACTORY__
#define __KAIROS_GRAPHICS_FACTORY__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include "GraphicsCommandQueue.h"
#include "GraphicsDevice.h"
#include "GraphicsSwapChain.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

typedef struct GraphicsFactory
{
	IDXGIFactory4* m_pFactory;

} GraphicsFactory;


KAIROS_EXPORT_BEGIN

int KAIROS_API GraphicsFactory_Create(GraphicsFactory* _this);

void KAIROS_API GraphicsFactory_Dispose(GraphicsFactory* _this);

int KAIROS_API GraphicsFactory_CreateDevice(GraphicsFactory* _this, GraphicsDevice* pGraphicsDevice);

int KAIROS_API GraphicsFactory_CreateSwapChain(GraphicsFactory* _this, GraphicsCommandQueue* pCommandQueue, GraphicsSwapChain* pGraphicsSwapChain, int width, int height, DXGI_FORMAT format, int msaa, int aaQuality, int bufferCount, HWND hwnd, BOOL windowed);

KAIROS_EXPORT_END

#endif
