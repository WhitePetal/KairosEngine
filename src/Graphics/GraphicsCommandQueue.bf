using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct GraphicsCommandQueue : IDisposable
	{
		public const int32 SUPPORT_MAX_EXECUTE_COMMAND_LIST_COUNT = 8;

		private void* m_pCommandQueue;

		public void Dispose() mut
		{
			if(m_pCommandQueue != null)
			 	GraphicsCommandQueue_Dispose(&this);
		}

		public void ExecuteCommandLists(GraphicsCommandList[] lists, int32 executeCount) mut
		{
			if(executeCount > SUPPORT_MAX_EXECUTE_COMMAND_LIST_COUNT)
			{
				GraphicsCommandList[] tempLists = scope GraphicsCommandList[SUPPORT_MAX_EXECUTE_COMMAND_LIST_COUNT];
				int32 batchCount = executeCount / SUPPORT_MAX_EXECUTE_COMMAND_LIST_COUNT;
				int32 modCount = batchCount % SUPPORT_MAX_EXECUTE_COMMAND_LIST_COUNT;

				int listIdx = 0;
				for(int32 batchIdx = 0; batchIdx < batchCount; ++batchIdx)
				{
					for(int32 i = 0; i < SUPPORT_MAX_EXECUTE_COMMAND_LIST_COUNT; ++i)
					{
						tempLists[i] = lists[listIdx++];
					}
					GraphicsCommandQueue_ExecuteCommandLists(&this, &tempLists[0], SUPPORT_MAX_EXECUTE_COMMAND_LIST_COUNT);
				}
				for(int32 modIdx = 0; modIdx < modCount; ++modIdx)
				{
					tempLists[modIdx] = lists[listIdx++];
				}
				GraphicsCommandQueue_ExecuteCommandLists(&this, &tempLists[0], modCount);
			}
			else
			{
				GraphicsCommandQueue_ExecuteCommandLists(&this, &lists[0], executeCount);
			}
		}

		public int32 Signal(ref GraphicsFence fence, uint64 fenceValue) mut
		{
			return GraphicsCommandQueue_Signal(&this, &fence, fenceValue);
		}
	}
}