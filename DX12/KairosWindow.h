#ifndef __KAIROS_WINDOW__
#define __KAIROS_WINDOW__

#include "KairosEngineDefines.h"
#include <Windows.h>

KAIROS_EXPORT_BEGIN

using KairosWndProcPtr = LRESULT(CALLBACK*)(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam);

HWND KAIROS_API KairosInitializeWindow(HINSTANCE hInstance, int ShowWnd, int width, int height, bool fullScreen, LPCWSTR windowName, LPCWSTR windowTitle, KairosWndProcPtr wndProc);

KAIROS_EXPORT_END

#endif // __KAIROS_WINDOW__

