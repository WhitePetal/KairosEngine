using System;
using ImGui;
using KairosEngine.Graphics;

namespace KairosEngine.ImGUI
{
	public struct ImGuiDescriptorHeapAllocator
	{
		public GraphicsDescriptorHeap Heap;
		public DescriptorHeapType HeapType;
		public DescriptorCpuHandle HeapStartCpu;
		public DescriptorGpuHandle HeapStartGpu;
		public uint32 HeapHandleIncrement;
		public ImGui.Vector<int32> FreeIndices;

		public void Create(GraphicsDevice device, GraphicsDescriptorHeap heap) mut
		{
			Heap = heap;
			var desc = Heap.GetDesc();
			HeapType = desc.Type;
			HeapStartCpu = Heap.GetCPUDescriptorHandleForHeapStart();
			HeapStartGpu = Heap.GetGPUDescriptorHandleForHeapStart();
			HeapHandleIncrement = device.GetDescriptorHandleIncrementSize(HeapType);
			int32 descriptorCount = int32(desc.NumDescriptors);
			FreeIndices.Reserve(descriptorCount);
			for(int32 n = descriptorCount; n > 0; --n)
				FreeIndices.PushBack(n - 1);
		}

		public void Destroy() mut
		{
			Heap = null;
			FreeIndices.Clear();
		}

		public void Alloc(ref DescriptorCpuHandle pOutCpuDescHandle, ref DescriptorGpuHandle pOutGpuDescHandle) mut
		{
			int32 idx = FreeIndices.Data[FreeIndices.Size - 1];
			FreeIndices.PopBack();
			pOutCpuDescHandle.Ptr = pOutCpuDescHandle.Ptr + (uint32(idx) * HeapHandleIncrement);
			pOutGpuDescHandle.Ptr = pOutCpuDescHandle.Ptr + (uint32(idx) * HeapHandleIncrement);
		}

		public void Free(DescriptorCpuHandle outCpuDescHandle, DescriptorGpuHandle outGpuDescHandle) mut
		{
			int32 cpu_idx = (int32)((outCpuDescHandle.Ptr - HeapStartCpu.Ptr) / HeapHandleIncrement);
			int32 gpu_idx = (int32)((outGpuDescHandle.Ptr - HeapStartGpu.Ptr) / HeapHandleIncrement);
			/*System.Diagnostics.Debug.Assert(cpu_idx == gpu_idx);*/
			FreeIndices.PushBack(cpu_idx);
		}
	}
}