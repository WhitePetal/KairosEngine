using KairosEngine.Graphics;

namespace KairosEngine.ImGUI
{
	public struct ImGuiDX12Data
	{
		public ImGuiDX12InitInfo InitInfo;
		public GraphicsFactory* pFactory;
		public GraphicsDevice* pDevice;
		public GraphicsRootSignature* pRootSignature;
		public GraphicsPipelineState* pPipelineState;
		public GraphicsCommandQueue* pCommandQueue;
		public bool CommandQueueOwned;
		public RenderTargetFormat RTVFormat;
		public RenderTargetFormat DSVFormat;
		public GraphicsDescriptorHeap* pSrvDescHeap;
		public GraphicsFence* pFence;
		public uint64 FenceLastSignaledValue;
		public GraphicsFenceEvent FenceEvent;
		public uint32 NumFramesInFlight;
		public bool TearingSupport;
		public bool LegacySingleDescriptorUsed;

		public GraphicsCommandAllocator* pTexCmdAllocator;
		public GraphicsCommandList* pTexCmdList;
		public GraphicsResource* pTexUploadBuffer;
		public uint32 pTexUploadBufferSize;
		public void* pTexUploadBufferMapped;

		public ImGuiDX12RenderBuffers* pFrameResources;
		public uint32 FrameIndex;
	}
}