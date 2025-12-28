namespace KairosEngine
{
	public static class Safty
	{
		public static mixin DELETE<T>(T obj) where T : delete, class
		{
			if(obj != null) delete obj;
			obj = null;
		}

		public static mixin DELETE<T>(T obj) where T : delete, struct
		{
			if(obj != null) delete obj;
			obj = default;
		}
	}
}