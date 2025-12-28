using System;

namespace ImGui
{
	extension ImGui
	{
		extension Vector<T>
		{
			[Inline]
			private int32 GrowCapacity(int32 sz)
			{
				int32 new_capacity = Capacity > 0 ? (Capacity + Capacity / 2) : 8;
				return new_capacity > sz ? new_capacity : sz;
			}

			[Inline]
			public void Resize(int32 new_size) mut
			{
				if(new_size > Capacity)
					Reserve(GrowCapacity(new_size));

				Size = new_size;
			}

			[Inline]
			public void Reserve(int32 new_capacity) mut
			{
				if(new_capacity <= Capacity)
					return;
				T* new_data = (T*)MemAllocImpl(uint64(new_capacity * sizeof(T)));
				if(Data != null)
				{
					Internal.MemCpy(new_data, Data, Size * sizeof(T));
					MemFreeImpl(Data);
				}
				Data = new_data;
				Capacity = new_capacity;
			}

			[Inline]
			public T* Insert(T* it, ref T v) mut
			{
				int64 off = it - Data;
				if(Size == Capacity)
					Reserve(GrowCapacity(Size + 1));
				if(off < Size)
					Internal.MemMove(Data + off + 1, Data + off, (Size - off) * sizeof(T));
				Internal.MemCpy(&Data[off], &v, sizeof(T));
				++Size;
				return Data + off;
			}

			[Inline]
			public void PushBack(ref T v) mut
			{
				if(Size == Capacity)
					Reserve(GrowCapacity(Size + 1));
				Data[Size++] = v;
			}

			[Inline]
			public void PushFront(ref T v) mut
			{
				if (Size == 0)
					PushBack(ref v);
				else
					Insert(Data, ref v); 
			}

			[Inline]
			public void PopBack() mut
			{
				--Size;
			}

			[Inline]
			public void Clear() mut
			{
				if(Data != null)
				{
					Size = Capacity = 0;
					MemFreeImpl(Data);
					Data = null;
				}
			}
		}
	}
}