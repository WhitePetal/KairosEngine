using System;

namespace KairosEngine.Graphics
{
	public class GraphicsCommandQueue
	{
		public const int32 SUPPORT_MAX_EXECUTE_COMMAND_LIST_COUNT = 8;

		public void* pInternalCommandQueue;

		public ~this()
		{
			if(pInternalCommandQueue != null)
			{
				GraphicsCommandQueue_Dispose(pInternalCommandQueue);
				pInternalCommandQueue = null;
			}
		}

		public void ExecuteCommandLists(GraphicsCommandList[] lists, int32 executeCount)
		{
			void*[] pCmdLists = scope void*[executeCount](?);
			for(int32 i = 0; i < executeCount; ++i)
			{
				pCmdLists[i] = lists[i].pInternalCommandList;
			}
			GraphicsCommandQueue_ExecuteCommandLists(pInternalCommandQueue, pCmdLists.Ptr, executeCount);
			// batch execute?
			/*if(executeCount > SUPPORT_MAX_EXECUTE_COMMAND_LIST_COUNT)
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
			}*/
		}

		public int32 Signal(GraphicsFence fence, uint64 fenceValue)
		{
			return GraphicsCommandQueue_Signal(pInternalCommandQueue, fence.pInternalFence, fenceValue);
		}
	}
}