using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsDevice : IDisposable
	{
		private void* m_pDevice;

		public void Dispose() mut
		{
			if(m_pDevice != null)
				GraphicsDevice_Dispose(&this);
		}

		public (int32 hr, GraphicsCommandQueue queue) CreateaCommandQueue(CommandListType type, int32 priority, CommandQueueFlags flags, uint32 nodeMask) mut
		{
			GraphicsCommandQueue queue = GraphicsCommandQueue();
			int32 hr = GraphicsDevice_CreateCommandQueue(&this, &queue, type, priority, flags, nodeMask);
			return (hr, queue);
		}

		public (int32 hr, GraphicsDescriptorHeap heap) CreateDescriptorHeap(int32 count, DescriptorHeapType type, DescriptorHeapFlags flags) mut
		{
			GraphicsDescriptorHeap heap = GraphicsDescriptorHeap();
			int32 hr = GraphicsDevice_CreateDescriptorHeap(&this, &heap, count, type, flags);
			return (hr, heap);
		}

		public uint32 GetDescriptorHandleIncrementSize(DescriptorHeapType type) mut
		{
			return GraphicsDevice_GetDescriptorHandleIncrementSize(&this, type);
		}

		public void CreateRenderTargetView(ref GraphicsRenderTarget renderTarget, GraphicsCPUDescriptorHandle handle) mut
		{
			GraphicsDevice_CreateRenderTargetView(&this, &renderTarget, handle.Ptr);
		}

		public void CreateRenderTargetView(ref GraphicsRenderTarget renderTarget, ref GraphicsDescriptorHeap descriptorHeap, int32 index) mut
		{
			GraphicsDevice_CreateRenderTargetView(&this, &renderTarget, &descriptorHeap, index);
		}

		public (int32 hr, GraphicsCommandAllocator commandAllocator) CreateCommandAllocator(CommandListType type) mut
		{
			GraphicsCommandAllocator commandAllocator = GraphicsCommandAllocator();
			int32 hr = GraphicsDevice_CreateCommandAllocator(&this, &commandAllocator, type);
			return (hr, commandAllocator);
		}

		public (int32 hr, GraphicsCommandList commandList) CreateCommandList(ref GraphicsCommandAllocator commandAllocator, CommandListType type, uint32 nodeMask) mut
		{
			GraphicsCommandList commandList = GraphicsCommandList();
			int32 hr = GraphicsDevice_CreateCommandList(&this, &commandAllocator, &commandList, type, nodeMask);
			return (hr, commandList);
		}

		public (int32 hr, GraphicsFence fence) CreateFence(uint64 initialValue, FenceFlags flags) mut
		{
			GraphicsFence fence = GraphicsFence();
			int32 hr = GraphicsDevice_CreateFence(&this, &fence, initialValue, flags);
			return (hr, fence);
		}

		public (int32 hr, GraphicsRootSignature rootSignature) CreateEmptyRootSignature(RootSignatureFlags flags) mut
		{
			GraphicsRootSignature rootSignature = GraphicsRootSignature();
			int32 hr = GraphicsDevice_CreateEmptyRootSignature(&this, &rootSignature, flags);
			return (hr, rootSignature);
		}

		public (int32 hr, GraphicsPipelineState pipelineState) CreatePipelineState(GraphicsInputLayoutElement[] inputlayouts, ref GraphicsRootSignature graphicsRootSignature,
			ref GraphicsShader graphicsVertexShader, ref GraphicsShader graphicsFragmentShader, TopologyType topologyType, RenderTargetFormat renderTargetFormat, uint32 msaa, uint32 aaQuality, uint32 sampleMask) mut
		{
			GraphicsPipelineState pipelineState = GraphicsPipelineState();
			int32 hr = GraphicsDevice_CreatePipelineState(&this, &pipelineState, inputlayouts.Ptr, 1, &graphicsRootSignature, &graphicsVertexShader, &graphicsFragmentShader, topologyType, renderTargetFormat, msaa, aaQuality, sampleMask);
			return (hr, pipelineState);
		}

		public (int32 hr, GraphicsBuffer buffer) CreateCommittedBufferResource(HeapType heapType, int resourceSize, HeapFlags heapFlags, ResourceStates resourceStates) mut
		{
			GraphicsBuffer buffer = GraphicsBuffer();
			int32 hr = GraphicsDevice_CreateCommittedBufferResource(&this, &buffer, heapType, (uint64)resourceSize, heapFlags, resourceStates);
			return (hr, buffer);
		}
	}
}