# ModemDeck project presentation

Prepared for human review. This copy describes the current v2 beta source and is not a notice that a store package or public beta has been published.

## English

### Short description

ModemDeck is an open-source Linux desktop app for inspecting and managing WWAN and mobile broadband modems, built with Rust and Slint.

### Full description

ModemDeck brings cellular modem status, radio diagnostics, supported-band controls, SIM and APN tools, traffic history, and modem troubleshooting into one Linux desktop application. It communicates with the host's ModemManager and NetworkManager services. The interface is built with Slint and the application logic is written in Rust.

Available controls depend on the modem model, firmware, ModemManager version, system services, and desktop session. Band changes use a confirmation and rollback flow; modem-specific features remain unavailable when the device does not expose a supported operation. The v2.0.0-beta.1 source is a prerelease candidate and still needs broader desktop and hardware validation.

### Feature summary

- Signal and serving-cell telemetry when exposed by the modem.
- Supported LTE/NR band selection, confirmation, and rollback protections.
- ModemManager-backed SIM tools and NetworkManager-backed APN profiles.
- Local traffic and signal history, diagnostics, and modem capability information.
- Optional Linux system-tray and GNOME Shell integration.
- English, Persian, Arabic, Turkish, German, French, Spanish, and Russian UI catalogs; in-context visual review is still required.

### Links and installation

- Source and issue tracker: https://github.com/MilMit/modemdeck-linux
- Project website: https://milmit.net
- Installation: use a validated package only when one is attached to a reviewed release. Until then, follow the repository's build instructions.

## فارسی

### معرفی کوتاه

ModemDeck یک برنامهٔ متن‌باز دسکتاپ لینوکس برای مشاهده و مدیریت مودم‌های WWAN و اینترنت همراه است که با Rust و Slint ساخته شده است.

### معرفی کامل

ModemDeck وضعیت مودم سلولی، اطلاعات رادیویی، کنترل باندهای پشتیبانی‌شده، ابزارهای سیم‌کارت و APN، تاریخچهٔ ترافیک و عیب‌یابی مودم را در یک برنامهٔ دسکتاپ لینوکس گرد هم می‌آورد. این برنامه با سرویس‌های ModemManager و NetworkManager سیستم کار می‌کند؛ رابط کاربری آن با Slint و منطق برنامه با Rust نوشته شده است.

دسترسی به هر قابلیت به مدل مودم، میان‌افزار، نسخهٔ ModemManager، سرویس‌های سیستم و نشست دسکتاپ بستگی دارد. تغییر باند با تأیید کاربر و امکان بازگردانی انجام می‌شود؛ قابلیت‌های وابسته به سازنده فقط هنگامی در دسترس‌اند که دستگاه عملیات پشتیبانی‌شده‌ای ارائه کند. کد نسخهٔ `v2.0.0-beta.1` پیش‌انتشار است و هنوز به آزمون گسترده‌تر روی سخت‌افزار و محیط‌های دسکتاپ نیاز دارد.

### ویژگی‌ها

- نمایش قدرت سیگنال و اطلاعات سلول در صورت ارائه توسط مودم.
- انتخاب باندهای LTE/NR پشتیبانی‌شده همراه با تأیید و سازوکار بازگردانی.
- ابزارهای سیم‌کارت مبتنی بر ModemManager و پروفایل‌های APN مبتنی بر NetworkManager.
- تاریخچهٔ محلی ترافیک و سیگنال، گزارش عیب‌یابی و اطلاعات سازگاری مودم.
- یکپارچه‌سازی اختیاری با System Tray لینوکس و GNOME Shell.
- کاتالوگ رابط کاربری به هشت زبان؛ بازبینی بصری ترجمه‌ها همچنان لازم است.

### پیوندها و نصب

- کد منبع و گزارش اشکال: https://github.com/MilMit/modemdeck-linux
- وب‌سایت پروژه: https://milmit.net
- نصب: پس از انتشار بازبینی‌شده، فقط از بستهٔ اعتبارسنجی‌شده استفاده کنید. تا آن زمان، دستورهای ساخت از کد منبع را در مخزن ببینید.
