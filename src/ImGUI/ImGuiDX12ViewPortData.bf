using KairosEngine.Graphics;

namespace KairosEngine.ImGUI
{
	public struct ImGuiDX12FrameContex
	{
		public uint64 FrenceValue;
		public GraphicsCommandAllocator CommandAllocator;
		public GraphicsRenderTarget RenderTarget;
		public DescriptorCpuHandle RenderTargetCpuDescriptors;
	}

	public class ImGuiDX12ViewPortData
	{
		public GraphicsCommandQueue CommandQueue;
		public GraphicsCommandList CommandList;
		public GraphicsDescriptorHeap RtvDescHeap;
		public GraphicsSwapChain SwapChain;
		public FenceEvent SwapChainWaitableObject;
		public uint32 NumFramesInFlight;
		public ImGuiDX12FrameContex[] FrameCtx;

		public uint32 FrameIndex;
		public ImGuiDX12RenderBuffers[] FrameRenderBuffers;


		public this(uint32 num_frames_in_flight)
		{
			CommandQueue = null;
			CommandList = null;
			RtvDescHeap = null;
			SwapChain = null;
			SwapChainWaitableObject = default;
			NumFramesInFlight = num_frames_in_flight;
			FrameCtx = new ImGuiDX12FrameContex[NumFramesInFlight](?);
			FrameIndex = 0;
			FrameRenderBuffers = new ImGuiDX12RenderBuffers[NumFramesInFlight](?);

			for(uint32 i = 0; i < NumFramesInFlight; ++i)
			{
				FrameCtx[i].FrenceValue = 0;
				FrameCtx[i].CommandAllocator = null;
				FrameCtx[i].RenderTarget = null;

				FrameRenderBuffers[i].IndexBuffer = null;
				FrameRenderBuffers[i].VertexBuffer = null;
				FrameRenderBuffers[i].VertexBufferSize = 5000;
				FrameRenderBuffers[i].IndexBufferSize = 10000;
			}
		}

		public ~this()
		{
			delete FrameCtx;
			delete FrameRenderBuffers;
		}
	}
}