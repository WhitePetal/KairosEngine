using KairosEngine.Graphics;

namespace KairosEngine.ImGUI
{
	public struct ImGuiDX12InitInfo
	{
		public GraphicsDevice Device;
		public GraphicsCommandQueue CommandQueue;
		public int32 NumFramesInFlight;
		public RenderTargetFormat RTVFormat;
		public DepthStencilFormat DSVFormat;
		public void* pUserData;

		public GraphicsDescriptorHeap SrvDescriptorHeap;
		public delegate void(ImGuiDX12InitInfo* pInfo, ref DescriptorCpuHandle pOutCpuHandle, ref DescriptorGpuHandle pOutGpuHandle) SrvDescriptorAllocFn;
		public delegate void(ImGuiDX12InitInfo* pInfo, DescriptorCpuHandle cpuHandle, DescriptorGpuHandle gpuHandle) SrvDescriptorFreeFn;
	}
}