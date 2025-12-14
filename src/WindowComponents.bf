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

		public int[] Ids;
		public Windows.HWnd[] Hwnds;
		public WindowFlags[] Flags;
		public int32_4[] Rects;

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

			Ids = new int[k_InitializeWindowCapacity];
			Hwnds = new Windows.HWnd[k_InitializeWindowCapacity];
			Flags = new WindowFlags[k_InitializeWindowCapacity];
			Rects = new int32_4[k_InitializeWindowCapacity];

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

		public int CreateWindow(int32_4 rect, bool fullScreen, Windows.HWnd hwnd)
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

			Ids[index] = id;
			Hwnds[index] = hwnd;
			Flags[index] = fullScreen ? (WindowFlags.Enable | WindowFlags.FullScreen) : WindowFlags.Enable;
			Rects[index] = rect;

			IdToIndex[id] = index;

			return id;
		}

		[DisableChecks]
		private void ExpandWindowContainers()
		{
			WindowCapacity = WindowCapacity << 1;

			var newIds = new int[WindowCapacity];
			var newHwnds = new Windows.HWnd[WindowCapacity];
			var newFlags = new WindowFlags[WindowCapacity];
			var newRects = new int32_4[WindowCapacity];

			for(int i = 0; i < WindowCount; ++i)
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