using System;
using System.Numerics;

namespace KairosEngine
{
	public class WindowComponents
	{
		public static WindowComponents Instance;

		public const int k_InitializeWindowCapacity = 8;
		public const int k_InitializeIdsCapacity = 8;

		public int WindowCount;
		public int WindowCapacity;

		public int32_4[] Rects;
		public WindowFlags[] Flags;
		public int[] Ids;

		private int m_IdsCount;
		private int m_IdsCapacity;
		public int[] IdToIndex;

		private int m_RecycleIdCount;
		private int m_RecycleIdCapacity;
		private int[] m_RecycleIds;

		private this()
		{
			WindowCount = 0;
			WindowCapacity = k_InitializeWindowCapacity;

			Rects = new int32_4[k_InitializeWindowCapacity];
			Flags = new WindowFlags[k_InitializeWindowCapacity];
			Ids = new int[k_InitializeWindowCapacity];

			m_IdsCount = 0;
			m_IdsCapacity = k_InitializeIdsCapacity;
			IdToIndex = new int[k_InitializeIdsCapacity];

			m_RecycleIdCount = 0;
			m_RecycleIdCapacity = k_InitializeIdsCapacity;
			m_RecycleIds = new int[k_InitializeIdsCapacity];
		}

		private ~this()
		{
			delete m_RecycleIds;

			delete IdToIndex;

			delete Ids;
			delete Flags;
			delete Rects;
		}

		[Inline]
		public static void Initialize()
		{
			Instance = new WindowComponents();
		}

		[Inline]
		public void DeInitialize()
		{
			delete Instance;
		}

		public int CreateWindow(int32_4 rect, bool fullScreen)
		{
			if(WindowCount >= WindowCapacity)
				ExpandWindowContainers();

			int id;
			if(m_RecycleIdCount > 0)
				id = m_RecycleIds[--m_RecycleIdCount];
			else
			{
				if(m_IdsCount >= m_IdsCapacity)
					ExpandIdToIndexContainers();

				id = m_IdsCount++;
			}

			int index = WindowCount++;

			Rects[index] = rect;
			Flags[index] = fullScreen ? (WindowFlags.Enable | WindowFlags.FullScreen) : WindowFlags.Enable;
			Ids[index] = id;

			IdToIndex[id] = index;

			return id;
		}

		[DisableChecks]
		private void ExpandWindowContainers()
		{
			WindowCapacity = WindowCapacity << 1;

			var newRects = new int32_4[WindowCapacity];
			var newFlags = new WindowFlags[WindowCapacity];
			var newIds = new int[WindowCapacity];

			for(int i = 0; i < WindowCount; ++i)
			{
				newRects[i] = Rects[i];
				newFlags[i] = Flags[i];
				newIds[i] = Ids[i];
			}

			delete Ids;
			delete Flags;
			delete Rects;

			Rects = newRects;
			Flags = newFlags;
			Ids = newIds;
		}

		[DisableChecks]
		private void ExpandIdToIndexContainers()
		{
			m_IdsCapacity = m_IdsCapacity << 1;

			var newIdToIndex = new int[m_IdsCapacity];

			for(int i = 0; i < m_IdsCount; ++i)
			{
				newIdToIndex[i] = IdToIndex[i];
			}

			delete IdToIndex;

			IdToIndex = newIdToIndex;
		}
	}
}