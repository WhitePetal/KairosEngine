using KairosEngine.Graphics;

namespace KairosEngine.ImGUI
{
	public struct ImGuiDX12InitInfo
	{
		public GraphicsDevice* pDevice;
		public GraphicsCommandQueue* pCommandQueue;
		public int32 NumFramesInFlight;
		public RenderTargetFormat RTVFormat;
		public RenderTargetFormat DSVFormat;
		public void* pUserData;

		public GraphicsDescriptorHeap* SrvDescriptorHeap;
		public delegate void(ImGuiDX12InitInfo* pInfo, GraphicsCPUDescriptorHandle* pOutCpuHandle, GraphicsGPUDescriptorHandle* pOutGpuHandle) SrvDescriptorAllocFn;
		public delegate void(ImGuiDX12InitInfo* pInfo, GraphicsCPUDescriptorHandle cpuHandle, GraphicsGPUDescriptorHandle gpuHandle) SrvDescriptorFreeFn;
	}
}