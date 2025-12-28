using KairosEngine.Graphics;

namespace KairosEngine.ImGUI
{
	public struct ImGuiDX12Data
	{
		public ImGuiDX12InitInfo InitInfo;
		public GraphicsFactory Factory;
		public GraphicsDevice Device;
		public GraphicsRootSignature RootSignature;
		public GraphicsPipelineState PipelineState;
		public GraphicsCommandQueue CommandQueue;
		public bool CommandQueueOwned;
		public RenderTargetFormat RTVFormat;
		public DepthStencilFormat DSVFormat;
		public GraphicsDescriptorHeap SrvDescHeap;
		public GraphicsFence Fence;
		public uint64 FenceLastSignaledValue;
		public FenceEvent FenceEvent;
		public uint32 NumFramesInFlight;
		public bool TearingSupport;
		public bool LegacySingleDescriptorUsed;

		public GraphicsCommandAllocator TexCmdAllocator;
		public GraphicsCommandList TexCmdList;
		public GraphicsResource TexUploadBuffer;
		public uint32 pTexUploadBufferSize;
		public void* pTexUploadBufferMapped;
	}
}