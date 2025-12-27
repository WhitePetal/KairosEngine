#include "GraphicsDescriptorHeap.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsDescriptorHeap_Dispose(ID3D12DescriptorHeap* _this)
{
	_this->Release();
}

D3D12_DESCRIPTOR_HEAP_DESC KAIROS_API GraphicsDescriptorHeap_GetDesc(ID3D12DescriptorHeap* _this)
{
	return _this->GetDesc();
}

D3D12_CPU_DESCRIPTOR_HANDLE KAIROS_API GraphicsDescriptorHeap_GetCPUDescriptorHandleForHeapStart(ID3D12DescriptorHeap* _this)
{
	return _this->GetCPUDescriptorHandleForHeapStart();
}

D3D12_GPU_DESCRIPTOR_HANDLE KAIROS_API GraphicsDescriptorHeap_GetGPUDescriptorHandleForHeapStart(ID3D12DescriptorHeap* _this)
{
	return _this->GetGPUDescriptorHandleForHeapStart();
}

KAIROS_EXPORT_END