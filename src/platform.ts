/** 前端侧运行平台，首屏即可读，不必等异步调用。
 *  需要权威值时用后端 get_app_info 的 platform 字段（std::env::consts::OS）。 */
export type Platform = "windows" | "macos" | "linux";

function detect(): Platform {
  const ua = navigator.userAgent;
  if (/windows/i.test(ua)) return "windows";
  if (/macintosh|mac os x/i.test(ua)) return "macos";
  return "linux";
}

export const PLATFORM: Platform = detect();
