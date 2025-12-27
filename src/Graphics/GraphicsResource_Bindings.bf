using System;

namespace KairosEngine.Graphics
{
	extension GraphicsResource
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsResource_Dispose")]
		private static extern void GraphicsResource_Dispose(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsResource_GetGPUVirtualAddress")]
		private static extern uint64 GraphicsResource_GetGPUVirtualAddress(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsResource_Unmap")]
		private static extern void GraphicsResource_Unmap(void* _this, uint32 subResource, uint64 begin, uint64 end);
	}
}