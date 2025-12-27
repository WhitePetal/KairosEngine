using System;

namespace KairosEngine.Graphics
{
	public struct FenceEvent
	{
		public const uint32 INFINITE_WAIT_TIME = 0xFFFFFFFF;

		public void* Handle;

		public int32 Create() mut
		{
			int32 hr = FenceEvent_Create(&Handle);
			return hr;
		}

		public void Close() mut
		{
			if(Handle != null)
			{
				FenceEvent_Close(Handle);
				Handle = null;
			}
		}

		public uint32 Wait(uint32 dwMilliseconds) mut
		{
			return FenceEvent_Wait(Handle, dwMilliseconds);
		}
	}
}