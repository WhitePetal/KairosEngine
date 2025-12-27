#ifndef __KAIROS_GRAPHICS_FACTORY__
#define __KAIROS_GRAPHICS_FACTORY__

#include "KairosEngineDefines.h"
#include "GraphicsCommandQueue.h"
#include "GraphicsDevice.h"
#include "GraphicsSwapChain.h"
#include <d3d12.h>
#include <dxgi1_5.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"


KAIROS_EXPORT_BEGIN

int KAIROS_API GraphicsFactory_Create(IDXGIFactory5** p_this);

void KAIROS_API GraphicsFactory_Dispose(IDXGIFactory5* _this);

int KAIROS_API GraphicsFactory_CreateDevice(IDXGIFactory5* _this, ID3D12Device** ppDevice);

int KAIROS_API GraphicsFactory_CreateSwapChain(IDXGIFactory5* _this, IDXGISwapChain3** ppSwapChain, ID3D12CommandQueue* pCommandQueue, int width, int height, DXGI_FORMAT format, int msaa, int aaQuality, int bufferCount, HWND hwnd, BOOL windowed);

int KAIROS_API GraphicsFactory_CheckFeatureSupport(IDXGIFactory5* _this, DXGI_FEATURE feature, void* pFeatureSupportData, UINT featureSupportDataSize);

KAIROS_EXPORT_END

#endif
