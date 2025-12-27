using System;

namespace KairosEngine.Graphics
{
	extension GraphicsDescriptorHeap
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDescriptorHeap_Dispose")]
		private static extern void GraphicsDescriptorHeap_Dispose(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDescriptorHeap_GetDesc")]
		private static extern DescriptorHeapDesc GraphicsDescriptorHeap_GetDesc(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDescriptorHeap_GetCPUDescriptorHandleForHeapStart")]
		private static extern DescriptorCpuHandle GraphicsDescriptorHeap_GetCPUDescriptorHandleForHeapStart(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDescriptorHeap_GetGPUDescriptorHandleForHeapStart")]
		private static extern DescriptorGpuHandle GraphicsDescriptorHeap_GetGPUDescriptorHandleForHeapStart(void* _this);
	}
}