using System;
using KairosEngine.Math;

namespace KairosEngine.Graphics
{
	extension GraphicsCommandList
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandList_Dispose")]
		private static extern void GraphicsCommandList_Dispose(GraphicsCommandList* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandList_UpdateSubResources")]
		private static extern int32 GraphicsCommandList_UpdateSubResources(GraphicsCommandList* _this, GraphicsResource* pDstResource, GraphicsResource* pFromResource,
			uint64 intermediatedOffset, uint32 firstSubResource, uint32 numSubResource, void* pData, int64 dataSize);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandList_ResourceBarrier")]
		private static extern void GraphicsCommandList_ResourceBarrier(GraphicsCommandList* _this, GraphicsResource* pResource, ResourceStates beforeStates, ResourceStates afterStates);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandList_Close")]
		private static extern int32 GraphicsCommandList_Close(GraphicsCommandList* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandList_Reset")]
		private static extern int32 GraphicsCommandList_Reset(GraphicsCommandList* _this, GraphicsCommandAllocator* pGraphicsCommandAllocator, GraphicsPipelineState* pGraphicsPipelineState);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandList_OMSetRenderTargets")]
		private static extern void GraphicsCommandList_OMSetRenderTargets(GraphicsCommandList* _this, GraphicsDescriptorHeap* pGraphicsDescriptorHeap,
			uint32 descriptorOffset, uint32 descriptorSize, uint32 descriptorCount);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandList_ClearRenderTargetView")]
		private static extern void GraphicsCommandList_ClearRenderTargetView(GraphicsCommandList* _this, GraphicsDescriptorHeap* pGraphicsDescriptorHeap,
			uint32 descriptorOffset, uint32 descriptorSize, float4* pColor, uint32 rectCount, Rect* pRects);
	}
}