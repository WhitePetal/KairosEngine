using System;

namespace KairosEngine.Graphics
{
	extension GraphicsDescriptorHeap
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDescriptorHeap_Dispose")]
		private static extern void GraphicsDescriptorHeap_Dispose(GraphicsDescriptorHeap* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDescriptorHeap_GetCPUDescriptorHandleForHeapStart")]
		private static extern uint64 GraphicsDescriptorHeap_GetCPUDescriptorHandleForHeapStart(GraphicsDescriptorHeap* _this);
	}
}