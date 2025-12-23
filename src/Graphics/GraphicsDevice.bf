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

		public (int hr, GraphicsCommandQueue queue) CreateaCommandQueue(CommandListType type, int priority, CommandQueueFlags flags, uint nodeMask) mut
		{
			GraphicsCommandQueue queue = GraphicsCommandQueue();
			int hr = GraphicsDevice_CreateCommandQueue(&this, &queue, type, priority, flags, nodeMask);
			return (hr, queue);
		}

		public (int hr, GraphicsDescriptorHeap heap) CreateDescriptorHeap(int count, DescriptorHeapType type, DescriptorHeapFlags flags) mut
		{
			GraphicsDescriptorHeap heap = GraphicsDescriptorHeap();
			int hr = GraphicsDevice_CreateDescriptorHeap(&this, &heap, count, type, flags);
			return (hr, heap);
		}

		public uint GetDescriptorHandleIncrementSize(DescriptorHeapType type) mut
		{
			return GraphicsDevice_GetDescriptorHandleIncrementSize(&this, type);
		}

		public void CreateRenderTargetView(ref GraphicsRenderTarget renderTarget, GraphicsCPUDescriptorHandle handle) mut
		{
			GraphicsDevice_CreateRenderTargetView(&this, &renderTarget, handle.Ptr);
		}

		public void CreateRenderTargetView(ref GraphicsRenderTarget renderTarget, ref GraphicsDescriptorHeap descriptorHeap, int index) mut
		{
			GraphicsDevice_CreateRenderTargetView(&this, &renderTarget, &descriptorHeap, index);
		}

		public (int hr, GraphicsCommandAllocator commandAllocator) CreateCommandAllocator(CommandListType type) mut
		{
			GraphicsCommandAllocator commandAllocator = GraphicsCommandAllocator();
			int hr = GraphicsDevice_CreateCommandAllocator(&this, &commandAllocator, type);
			return (hr, commandAllocator);
		}

		public (int hr, GraphicsCommandList commandList) CreateCommandList(ref GraphicsCommandAllocator commandAllocator, CommandListType type, uint nodeMask) mut
		{
			GraphicsCommandList commandList = GraphicsCommandList();
			int hr = GraphicsDevice_CreateCommandList(&this, &commandAllocator, &commandList, type, nodeMask);
			return (hr, commandList);
		}

		public (int hr, GraphicsFence fence) CreateFence(uint64 initialValue, FenceFlags flags) mut
		{
			GraphicsFence fence = GraphicsFence();
			int hr = GraphicsDevice_CreateFence(&this, &fence, initialValue, flags);
			return (hr, fence);
		}

		public (int hr, GraphicsRootSignature rootSignature) CreateEmptyRootSignature(RootSignatureFlags flags) mut
		{
			GraphicsRootSignature rootSignature = GraphicsRootSignature();
			int hr = GraphicsDevice_CreateEmptyRootSignature(&this, &rootSignature, flags);
			return (hr, rootSignature);
		}
	}
}