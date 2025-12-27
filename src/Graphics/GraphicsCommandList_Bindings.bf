using System;
using KairosEngine.Math;

namespace KairosEngine.Graphics
{
	extension GraphicsCommandList
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandList_Dispose")]
		private static extern void GraphicsCommandList_Dispose(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandList_UpdateSubResources")]
		private static extern uint64 GraphicsCommandList_UpdateSubResources(void* _this, void* pDstResource, void* pFromResource,
			uint64 intermediatedOffset, uint32 firstSubResource, uint32 numSubResource, void* pData, int64 dataSize);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandList_ResourceBarrier")]
		private static extern void GraphicsCommandList_ResourceBarrier(void* _this, void* pResource, ResourceStates beforeStates, ResourceStates afterStates);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandList_Close")]
		private static extern int32 GraphicsCommandList_Close(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandList_Reset")]
		private static extern int32 GraphicsCommandList_Reset(void* _this, void* pCommandAllocator, void* pPipelineState);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandList_OMSetRenderTargets")]
		private static extern void GraphicsCommandList_OMSetRenderTargets(void* _this, void* pDescriptorHeap,
			uint32 descriptorOffset, uint32 descriptorSize, uint32 descriptorCount);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandList_ClearRenderTargetView")]
		private static extern void GraphicsCommandList_ClearRenderTargetView(void* _this, void* pGraphicsDescriptorHeap, uint32 descriptorOffset, uint32 descriptorSize, float4* pColor, uint32 rectCount, Rect* pRects);
	}
}