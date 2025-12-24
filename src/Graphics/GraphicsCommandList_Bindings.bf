using System;

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
	}
}