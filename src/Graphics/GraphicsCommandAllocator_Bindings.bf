using System;

namespace KairosEngine.Graphics
{
	extension GraphicsCommandAllocator
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandAllocator_Dispose")]
		private static extern void GraphicsCommandAllocator_Dispose(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandAllocator_Reset")]
		private static extern int32 GraphicsCommandAllocator_Reset(void* _this);
	}
}