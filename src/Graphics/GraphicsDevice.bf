using System;

namespace KairosEngine.Graphics
{
	public class GraphicsDevice
	{
		public void* pInternalDevice;

		public ~this()
		{
			if(pInternalDevice != null)
			{
				GraphicsDevice_Dispose(pInternalDevice);
				pInternalDevice = null;
			}
		}

		public (int32 hr, GraphicsCommandQueue queue) CreateaCommandQueue(CommandListType type, int32 priority, CommandQueueFlags flags, uint32 nodeMask)
		{
			GraphicsCommandQueue queue = new GraphicsCommandQueue();
			int32 hr = GraphicsDevice_CreateCommandQueue(pInternalDevice, &queue.pInternalCommandQueue, type, priority, flags, nodeMask);
			return (hr, queue);
		}

		public (int32 hr, GraphicsDescriptorHeap heap) CreateDescriptorHeap(int32 count, DescriptorHeapType type, DescriptorHeapFlags flags)
		{
			GraphicsDescriptorHeap heap = new GraphicsDescriptorHeap();
			int32 hr = GraphicsDevice_CreateDescriptorHeap(pInternalDevice, &heap.pInternalDescriptorHeap, count, type, flags);
			return (hr, heap);
		}

		public uint32 GetDescriptorHandleIncrementSize(DescriptorHeapType type)
		{
			return GraphicsDevice_GetDescriptorHandleIncrementSize(pInternalDevice, type);
		}

		public void CreateRenderTargetView(GraphicsRenderTarget renderTarget, DescriptorCpuHandle handle)
		{
			GraphicsDevice_CreateRenderTargetView(pInternalDevice, renderTarget.pInternalResource, handle);
		}

		public void CreateRenderTargetView(GraphicsRenderTarget renderTarget, GraphicsDescriptorHeap descriptorHeap, int32 index)
		{
			GraphicsDevice_CreateRenderTargetView(pInternalDevice, renderTarget.pInternalResource, descriptorHeap.pInternalDescriptorHeap, index);
		}

		public (int32 hr, GraphicsCommandAllocator commandAllocator) CreateCommandAllocator(CommandListType type)
		{
			GraphicsCommandAllocator commandAllocator = new GraphicsCommandAllocator();
			int32 hr = GraphicsDevice_CreateCommandAllocator(pInternalDevice, &commandAllocator.pInternalCommandAllocator, type);
			return (hr, commandAllocator);
		}

		public (int32 hr, GraphicsCommandList commandList) CreateCommandList(GraphicsCommandAllocator commandAllocator, CommandListType type, uint32 nodeMask)
		{
			GraphicsCommandList commandList = new GraphicsCommandList();
			int32 hr = GraphicsDevice_CreateCommandList(pInternalDevice, &commandList.pInternalCommandList, commandAllocator.pInternalCommandAllocator, type, nodeMask);
			return (hr, commandList);
		}

		public (int32 hr, GraphicsFence fence) CreateFence(uint64 initialValue, FenceFlags flags)
		{
			GraphicsFence fence = new GraphicsFence();
			int32 hr = GraphicsDevice_CreateFence(pInternalDevice, &fence.pInternalFence, initialValue, flags);
			return (hr, fence);
		}

		public (int32 hr, GraphicsRootSignature rootSignature) CreateEmptyRootSignature(RootSignatureFlags flags)
		{
			GraphicsRootSignature rootSignature = new GraphicsRootSignature();
			int32 hr = GraphicsDevice_CreateEmptyRootSignature(pInternalDevice, &rootSignature.pInternalRootSignature, flags);
			return (hr, rootSignature);
		}

		public (int32 hr, GraphicsRootSignature rootSignature) CreateRootSignature(ref RootSignatureDesc desc)
		{
			GraphicsRootSignature rootSignature = new GraphicsRootSignature();
			int32 hr = GraphicsDevice_CreateRootSignature(pInternalDevice, &rootSignature.pInternalRootSignature, &desc);
			return (hr, rootSignature);
		}

		public (int32 hr, GraphicsPipelineState pipelineState) CreatePipelineState(InputLayoutElement[] inputlayouts, GraphicsRootSignature graphicsRootSignature,
			GraphicsShader graphicsVertexShader, GraphicsShader graphicsFragmentShader, TopologyType topologyType, RenderTargetFormat renderTargetFormat, uint32 msaa, uint32 aaQuality, uint32 sampleMask)
		{
			GraphicsPipelineState pipelineState = new GraphicsPipelineState();
			int32 hr = GraphicsDevice_CreatePipelineState(pInternalDevice, &pipelineState.pInternalPipelineState, inputlayouts.Ptr, 1, graphicsRootSignature.pInternalRootSignature,
				graphicsVertexShader.pInternalShader, graphicsFragmentShader.pInternalShader, topologyType, renderTargetFormat, msaa, aaQuality, sampleMask);
			return (hr, pipelineState);
		}

		public (int32 hr, GraphicsResource buffer) CreateCommittedBufferResource(HeapType heapType, int resourceSize, HeapFlags heapFlags, ResourceStates resourceStates)
		{
			GraphicsResource buffer = new GraphicsResource();
			int32 hr = GraphicsDevice_CreateCommittedBufferResource(pInternalDevice, &buffer.pInternalResource, heapType, (uint64)resourceSize, heapFlags, resourceStates);
			return (hr, buffer);
		}
	}
}