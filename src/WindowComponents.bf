using System;
using System.Numerics;
using System.Collections;

namespace KairosEngine
{
	public class WindowComponents
	{
		public static WindowComponents Instance;

		public const int32 k_InitializeWindowCapacity = 8;
		public const int32 k_InitializeIdsCapacity = 8;

		public int32 WindowCount;
		public int32 WindowCapacity;

		public int32[] Ids;
		public Windows.HWnd[] Hwnds;
		public WindowFlags[] Flags;
		public int32_4[] Rects;

		private int32 m_IdsCount;
		private int32 m_IdsCapacity;
		public int32[] IdToIndex;

		private int32 m_RecycleIdCount;
		private int32 m_RecycleIdCapacity;
		private int32[] m_RecycleIds;

		private this()
		{
			WindowCount = 0;
			WindowCapacity = k_InitializeWindowCapacity;

			Ids = new int32[k_InitializeWindowCapacity];
			Hwnds = new Windows.HWnd[k_InitializeWindowCapacity];
			Flags = new WindowFlags[k_InitializeWindowCapacity];
			Rects = new int32_4[k_InitializeWindowCapacity];

			m_IdsCount = 0;
			m_IdsCapacity = k_InitializeIdsCapacity;
			IdToIndex = new int32[k_InitializeIdsCapacity];

			m_RecycleIdCount = 0;
			m_RecycleIdCapacity = k_InitializeIdsCapacity;
			m_RecycleIds = new int32[k_InitializeIdsCapacity];
		}

		private ~this()
		{
			delete m_RecycleIds;

			delete IdToIndex;

			delete Rects;
			delete Flags;
			delete Hwnds;
			delete Ids;
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

		public int32 CreateWindow(int32_4 rect, bool fullScreen, Windows.HWnd hwnd)
		{
			if(WindowCount >= WindowCapacity)
				ExpandWindowContainers();

			int32 id;
			if(m_RecycleIdCount > 0)
				id = m_RecycleIds[--m_RecycleIdCount];
			else
			{
				if(m_IdsCount >= m_IdsCapacity)
					ExpandIdToIndexContainers();

				id = m_IdsCount++;
			}

			int32 index = WindowCount++;

			Ids[index] = id;
			Hwnds[index] = hwnd;
			Flags[index] = fullScreen ? (WindowFlags.Enable | WindowFlags.FullScreen) : WindowFlags.Enable;
			Rects[index] = rect;

			IdToIndex[id] = index;

			return id;
		}

		public void DestroyWindow(int32 id)
		{
			int32 index = IdToIndex[id];
			int32 end = --WindowCount;

			int32 endId = Ids[end];

			Ids[index] = endId;
			Hwnds[index] = Hwnds[end];
			Flags[index] = Flags[end];
			Rects[index] = Rects[end];

			IdToIndex[endId] = index;

			if(m_RecycleIdCount >= m_RecycleIdCapacity)
				ExpandRecyleIdContainers();

			m_RecycleIds[m_RecycleIdCount++] = id;
		}

		[DisableChecks]
		private void ExpandWindowContainers()
		{
			WindowCapacity = WindowCapacity << 1;

			var newIds = new int32[WindowCapacity];
			var newHwnds = new Windows.HWnd[WindowCapacity];
			var newFlags = new WindowFlags[WindowCapacity];
			var newRects = new int32_4[WindowCapacity];

			for(int32 i = 0; i < WindowCount; ++i)
			{
				newIds[i] = Ids[i];
				newHwnds[i] = Hwnds[i];
				newFlags[i] = Flags[i];
				newRects[i] = Rects[i];
			}

			delete Rects;
			delete Flags;
			delete Hwnds;
			delete Ids;


			Ids = newIds;
			Hwnds = newHwnds;
			Flags = newFlags;
			Rects = newRects;
		}

		[DisableChecks]
		private void ExpandIdToIndexContainers()
		{
			m_IdsCapacity = m_IdsCapacity << 1;

			var newIdToIndex = new int32[m_IdsCapacity];

			for(int32 i = 0; i < m_IdsCount; ++i)
			{
				newIdToIndex[i] = IdToIndex[i];
			}

			delete IdToIndex;

			IdToIndex = newIdToIndex;
		}

		[DisableChecks]
		private void ExpandRecyleIdContainers()
		{
			m_RecycleIdCapacity = m_RecycleIdCapacity << 1;

			var newRecyleIds = new int32[m_RecycleIdCapacity];

			for(int32 i = 0; i < m_RecycleIdCount; ++i)
			{
				newRecyleIds[i] = m_RecycleIds[i];
			}

			delete m_RecycleIds;

			m_RecycleIds = newRecyleIds;
		}
	}
}