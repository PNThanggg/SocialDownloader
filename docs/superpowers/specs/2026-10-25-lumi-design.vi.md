# Lumi — Thiết kế bản đầu (kiểu A: dán link)

Ngày: 2026-10-25 · Trạng thái: **đã duyệt** (người dùng duyệt 2026-10-25)

> **Ngôn ngữ:** Tiếng Việt — bản để đọc. Claude triển khai theo bản tiếng Anh `2026-10-25-lumi-design.md`. Nếu hai bản khác nhau thì bản tiếng Anh được tính. Khi sửa thì sửa cả hai.

Ký hiệu: **[Người dùng]** = người dùng đã quyết. **[Claude]** = quyết định của người viết tài liệu, người dùng chưa xác nhận, người làm có thể đổi nếu có lý do. Không đánh dấu = mô tả chung.

## 1. Mục đích và lý do

Công cụ desktop cá nhân trên Windows để lấy media từ mạng xã hội về máy mà không phải đi tìm trang tải trung gian. Hai việc thật đã xảy ra **[Người dùng]**:

- Thấy một video TikTok nhạc hay, muốn có file **mp3** để nghe, nhưng lười đi tìm nguồn nhạc.
- Muốn lưu **ảnh Instagram** về máy.

Nguồn cần hỗ trợ: TikTok, Instagram, YouTube, Facebook **[Người dùng]**. Các trang khác mà yt-dlp/gallery-dl đọc được thì cứ cho chạy, nhưng không cần kiểm thử riêng.

Thành công nghĩa là: copy link → dán vào Lumi → vài giây sau thấy các lựa chọn → bấm Tải → chọn chỗ lưu → có file. Không cần mở trình duyệt, không có quảng cáo, không cần trang web trung gian.

## 2. Mục tiêu, ngoài phạm vi, điều cần tránh

**Mục tiêu**
- Dán 1 link → hiện **link gốc** (raw/direct URL) cho từng định dạng có sẵn: MP3 (âm thanh), MP4 (từng chất lượng), ảnh **[Người dùng]**.
- Mỗi dòng có nút **Copy** (copy link gốc) và nút **Tải** (tải file về máy) **[Người dùng]**.
- Khi bấm Tải thì mở **hộp thoại "Save as" của Windows** để chọn nơi lưu **[Người dùng]**.
- Kết nối tài khoản Instagram và Facebook bằng **cửa sổ đăng nhập ngay trong app** cho các bài cần đăng nhập **[Người dùng]**.
- Giao diện đẹp, file cài nhỏ **[Người dùng]**.

**Ngoài phạm vi bản đầu** **[Claude]**, người dùng chưa phản đối
- Tải cả playlist/kênh, tải nhiều link cùng lúc, hàng đợi.
- Lịch sử tải.
- macOS/Linux. Chỉ làm Windows 10/11 x64.
- Chế độ lướt Facebook/Instagram trong app (kiểu B). Phần này có tài liệu riêng: `2026-10-25-lumi-browse-mode-design.vi.md` **[Người dùng]**. Bản A phải chừa đường để gắn kiểu B vào (xem §8).
- Tự cập nhật bản thân app Lumi. Chỉ cập nhật các công cụ bên trong (xem §5).

**Điều cần tránh, dù app vẫn "chạy được"** **[Claude]**
- File cài nặng, mở app chậm.
- Dán link xong không có dấu hiệu gì là app đang xử lý.
- Lỗi hiện ra dạng stack trace hoặc log thô của yt-dlp. Mọi lỗi phải được dịch sang lời dễ hiểu và chỉ cách xử lý (xem §6).
- Lộ cookie đăng nhập của người dùng.

## 3. Cách làm (công nghệ)

| Phần | Lựa chọn | Ai quyết |
|---|---|---|
| Vỏ app + giao diện | **Tauri 2** (Rust + WebView2) | [Người dùng] |
| Đọc link, tải video/âm thanh | **yt-dlp** (bản `yt-dlp.exe` độc lập) chạy như tiến trình con | [Claude], người dùng đã nghe và đồng ý hướng này |
| Đổi sang MP3, ghép video+audio | **ffmpeg** | [Claude] |
| Ảnh (bài ảnh Instagram, slideshow ảnh TikTok, ảnh Facebook) | **gallery-dl** (bản `.exe` độc lập) | [Claude]. Lý do: yt-dlp chỉ làm video, bài chỉ có ảnh trên Instagram thường không đọc được. Người dùng chưa được hỏi về điểm này. |
| Frontend | Svelte 5 + TypeScript + Vite | [Claude]. Bundle nhỏ. Người làm có thể đổi sang framework khác nếu bundle vẫn nhỏ. |
| Ngôn ngữ giao diện | **Tiếng Việt + tiếng Anh** | [Người dùng]. *[Claude]* Mặc định theo ngôn ngữ của Windows (máy tiếng Việt thì dùng tiếng Việt, còn lại dùng tiếng Anh). Đổi được trong Cài đặt. Mọi chuỗi giao diện và câu báo lỗi (§6) đều có đủ hai thứ tiếng. |

Python, Go và Rust thuần đã được cân nhắc. Chọn Tauri vì yt-dlp không có bản thay thế tương đương trên Go/Rust, nên đằng nào bên trong cũng phải chạy yt-dlp. Khi đó vỏ Tauri là cách nhẹ nhất để có giao diện đẹp.

## 4. Màn hình và hành vi

Người dùng đã chọn qua mockup: **bố cục A, một màn hình gọn** và **phong cách kính mờ** **[Người dùng]**.

### 4.1 Màn hình chính
- Ở giữa có tiêu đề "Dán link vào đây" và dòng phụ "TikTok · Instagram · YouTube · Facebook".
- Ô nhập link và nút **Lấy link**. Nhấn Enter cũng chạy.
- *[Claude]* Khi mở app hoặc khi cửa sổ được focus lại, nếu clipboard đang chứa link của một trang được hỗ trợ thì hiện gợi ý nhỏ "Dán link vừa copy?". Không tự chạy khi người dùng chưa đồng ý.
- Khi đang xử lý: nút chuyển sang trạng thái loading, khung kết quả hiện skeleton. Không được để màn hình đứng im.
- Góc phải trên có nút bánh răng để mở **Cài đặt** (§4.5).

### 4.2 Khung kết quả
- Phần đầu: ảnh thu nhỏ, tiêu đề, tác giả, thời lượng (nếu có).
- Bên dưới là danh sách dòng. Mỗi dòng gồm nhãn định dạng (MP3 / MP4 / JPG…), thông tin (chất lượng, dung lượng ước tính nếu biết), nút **Copy**, nút **Tải**.
- **Video:** liệt kê các mức chất lượng có thật, *[Claude]* gộp thành các mức dễ hiểu (1080p, 720p, 480p…), không đổ ra toàn bộ danh sách format của yt-dlp.
- **MP3:** luôn có một dòng MP3 nếu nguồn có âm thanh. *[Claude]* Mặc định chất lượng tốt nhất (`--audio-quality 0`).
- **Nhạc gốc TikTok** **[Người dùng]**: video TikTok thường dùng một bài nhạc gốc (sound). Bài này có thể khác âm thanh của video, vì video hay có giọng nói chèn lên hoặc chỉ cắt một đoạn. Khi TikTok có thông tin nhạc gốc thì hiện **thêm một dòng riêng "Nhạc gốc"** (tên bài + nghệ sĩ) bên cạnh dòng MP3 của video. Dòng này cũng có Copy/Tải, tải ra MP3. Nếu không lấy được nhạc gốc thì không hiện dòng này, cũng không báo lỗi.
  - yt-dlp không đưa nhạc gốc thành một format. Người làm tự chọn cách lấy: đọc dữ liệu bài viết của TikTok (ví dụ trường `music.playUrl` trong dữ liệu trang/API), dùng gallery-dl, hoặc cách khác. Phải kiểm chứng sớm vì đây là việc chính người dùng cần.
  - *[Claude]* Gắn tag ID3 (tiêu đề, nghệ sĩ, ảnh bìa nếu có) cho file MP3 của dòng nhạc gốc. Dòng MP3 của video thì gắn tiêu đề video và tác giả.
- **Bài nhiều ảnh (carousel):** *[Claude]* hiện lưới ảnh thu nhỏ, mỗi ảnh có Copy/Tải riêng, thêm nút **Tải tất cả**. "Tải tất cả" mở hộp thoại chọn **thư mục** thay vì chọn file.
- **Bài trộn ảnh + video:** mỗi phần tử là một ô trong lưới, ô video có biểu tượng play và có cả lựa chọn MP4/MP3.

### 4.3 Copy link gốc
- Copy là copy URL trực tiếp mà yt-dlp/gallery-dl trả về. Người dùng chỉ cần link này **để xem cho biết** **[Người dùng]**. Không cần đảm bảo link mở được ngoài app, không cần proxy hay link trung gian.
- *[Claude]* Phải nói rõ trong UI (tooltip hoặc dòng nhỏ) rằng link gốc **có thể hết hạn sau vài giờ và có thể không mở được trong trình duyệt** (CDN của TikTok/Instagram hay đòi header/cookie). Nút Tải trong app không bị ảnh hưởng.
- Với MP3: không có "link gốc MP3" vì MP3 được tạo trên máy. *[Claude]* Nút Copy của dòng MP3 copy link luồng âm thanh gốc (thường là m4a/webm). Nếu nguồn chỉ có video gộp sẵn âm thanh (thường gặp ở TikTok) thì copy link video đó. Tooltip ghi rõ là đang copy file gốc nào.
- Với video YouTube chất lượng cao (video và âm thanh tách riêng): *[Claude]* Copy chỉ copy được luồng video, và phải ghi chú "không có tiếng, bấm Tải để có file đầy đủ". Nếu có format gộp sẵn thì ưu tiên copy format đó.

### 4.4 Tải
- Bấm **Tải** → mở hộp thoại Save as của Windows **[Người dùng]**.
  - *[Claude]* Tên file điền sẵn theo tiêu đề (đã bỏ ký tự không hợp lệ). Thư mục mặc định là thư mục lần trước người dùng chọn. Lần đầu dùng thư mục Downloads.
- Trong lúc tải, nút Tải biến thành thanh tiến trình (%) và có nút hủy. Xong thì hiện nút **Mở thư mục**. *[Claude]*
- *[Claude]* Mỗi lúc chỉ chạy một lượt tải cho mỗi dòng. Các dòng khác vẫn bấm được (không cần hàng đợi thật).
- Tải bằng chính yt-dlp/gallery-dl (có cookie nếu đã kết nối). Không tự tải bằng HTTP từ link gốc, để giữ đúng header/cookie và tự xử lý ghép video+audio.
- *[Claude]* Phía Rust quản lý các lượt tải và tiến trình của chúng (mỗi lượt có job id), rồi phát sự kiện tiến trình cho mọi cửa sổ. Lượt tải không gắn với cửa sổ đã bắt đầu nó. Kiểu B cần điều này vì tải phải tiếp tục chạy sau khi popup đóng.

### 4.5 Cài đặt (sau nút bánh răng)
- **Tài khoản:** hai dòng "Instagram" và "Facebook". Mỗi dòng có trạng thái (đã kết nối / chưa), nút **Kết nối** và nút **Đăng xuất** **[Người dùng]** chọn đăng nhập trong app.
- **Công cụ:** phiên bản yt-dlp, gallery-dl, ffmpeg, kèm nút **Cập nhật**.
- **Ngôn ngữ:** Tiếng Việt / English **[Người dùng]**.
- *[Claude]* Không có cài đặt chất lượng mặc định ở bản đầu.

### 4.6 Phong cách hình ảnh **[Người dùng]** chọn "kính mờ"
- Cửa sổ dùng hiệu ứng **Mica** (Windows 11) qua window effects của Tauri, cửa sổ trong suốt. *[Claude]* Trên Windows 10 không có Mica thì chuyển sang Acrylic, nếu vẫn không được thì dùng nền gradient tối có ánh màu như mockup.
- Các khối (ô nhập, khung kết quả, dòng định dạng) dùng nền trắng trong suốt khoảng 6–12% + viền trắng mờ + `backdrop-filter: blur`.
- Nút chính màu trắng gần đặc, chữ tối. Nhãn định dạng có màu riêng: MP3 xanh lá, MP4 xanh dương, ảnh cam.
- *[Claude]* Thanh tiêu đề tùy chỉnh (frameless), có nút thu nhỏ/đóng, kéo được.
- Mockup tham chiếu: `.superpowers/brainstorm/*/content/style.html`, lựa chọn "glass". Thư mục này không được commit vào git. Người làm chỉ cần mô tả ở trên.

## 5. Các công cụ bên trong: tải và cập nhật

Mục tiêu file cài nhỏ **[Người dùng]**. Vì vậy **[Claude]**:
- File cài **không** chứa sẵn yt-dlp, gallery-dl, ffmpeg.
- **Lần mở đầu tiên:** tải `yt-dlp.exe` và `gallery-dl.exe` từ trang GitHub Releases chính thức vào `%LOCALAPPDATA%\Lumi\bin\`, có màn hình "Đang chuẩn bị…" kèm tiến trình.
- **ffmpeg** (khoảng 80MB) chỉ tải **khi cần lần đầu** (lần đầu tải MP3 hoặc video cần ghép). Hiện thông báo "Cần tải thêm bộ chuyển đổi (~80MB) một lần". Nguồn gợi ý: bản build Windows từ `yt-dlp/FFmpeg-Builds` hoặc gyan.dev (bản essentials).
- **YouTube cần thêm JavaScript runtime:** yt-dlp bản hiện tại cần một JS runtime bên ngoài (ví dụ Deno) mới lấy đủ format của YouTube. *[Claude]* Runtime này cũng được tải **khi cần lần đầu** (link YouTube đầu tiên), giống ffmpeg, và có thông báo dung lượng. Nếu chưa có runtime hoặc tải thất bại thì vẫn chạy với số format ít hơn, kèm ghi chú "chưa có đủ chất lượng".
- Kiểm tra checksum (SHA-256) nếu nguồn có công bố.
- **Cập nhật:** nút trong Cài đặt. *[Claude]* Ngoài ra, nếu yt-dlp/gallery-dl báo lỗi đọc link (extractor error) thì gợi ý "Thử cập nhật công cụ" ngay trong thông báo lỗi. Không tự cập nhật ngầm.
- Không có mạng ở lần mở đầu: báo rõ và có nút thử lại.

## 6. Lỗi: phải nói bằng lời dễ hiểu

Lớp Rust nhận lỗi từ yt-dlp/gallery-dl, phân loại, rồi trả về một mã lỗi cho UI. UI hiện câu theo ngôn ngữ đang chọn kèm nút hành động. Tối thiểu cần có **[Claude]** (câu tiếng Anh tương ứng và đầy đủ các key nằm ở Task 11 của kế hoạch):

| Tình huống | Câu hiện ra | Nút |
|---|---|---|
| Link không thuộc trang nào hỗ trợ / không phải link | "Link này Lumi chưa đọc được." | — |
| Cần đăng nhập (IG/FB) | "Bài này cần đăng nhập Instagram." | **Kết nối Instagram** |
| Bài riêng tư / đã xóa | "Bài này riêng tư hoặc đã bị xóa." | — |
| Extractor hỏng (trang đổi cấu trúc) | "Trang này vừa thay đổi, thử cập nhật công cụ." | **Cập nhật** |
| Mất mạng | "Không có kết nối mạng." | **Thử lại** |
| Bị chặn tạm (rate limit) | "Trang đang chặn tạm, thử lại sau ít phút." | — |
| Lỗi khác | "Có lỗi khi đọc link." | **Xem chi tiết** (mở log thô) |

## 7. Kết nối tài khoản (đăng nhập trong app)

**[Người dùng]** chọn đăng nhập ngay trong app (người dùng đang dùng IG/FB trên Chrome, nhưng Chrome bản mới khóa việc đọc cookie từ bên ngoài, nên không mượn cookie từ Chrome).

Luồng hoạt động:
1. Bấm **Kết nối Instagram** → mở một cửa sổ Tauri mới (WebView2) tải `https://www.instagram.com/accounts/login/`.
2. Người dùng đăng nhập bình thường (kể cả 2FA).
3. App phát hiện đã đăng nhập xong: cookie phiên có mặt (`sessionid` với Instagram, `c_user` + `xs` với Facebook) hoặc URL đã rời trang login. Sau đó đọc cookie của domain đó, **gồm cả HttpOnly**, qua API cookie của Tauri/WebView2 (`Webview::cookies_for_url`, có từ Tauri ~2.4, PR tauri-apps/tauri#12665), rồi đóng cửa sổ.
   - *Rủi ro cần kiểm chứng sớm:* API này có lấy đủ cookie HttpOnly của domain `https` bên ngoài trên Windows hay không. Nếu không, phương án dự phòng là gọi thẳng `ICoreWebView2CookieManager::GetCookies` qua crate `webview2-com`.
4. Ghi cookie ra file định dạng Netscape `cookies.txt` (định dạng yt-dlp/gallery-dl đọc được). **[Claude]** Lúc nghỉ thì mã hóa file bằng Windows DPAPI (`CryptProtectData`, phạm vi user hiện tại). Chỉ giải mã ra file tạm khi chạy yt-dlp/gallery-dl, chạy xong thì xóa file tạm.
5. Khi đọc link IG/FB: nếu đã kết nối thì truyền `--cookies <file>` cho yt-dlp và `--cookies <file>` cho gallery-dl.
6. **Đăng xuất** = xóa file cookie + xóa dữ liệu WebView của cửa sổ đăng nhập.

**[Claude]** Cửa sổ đăng nhập của mỗi trang dùng **thư mục dữ liệu WebView riêng của trang đó** (profile theo từng trang): `%LOCALAPPDATA%\Lumi\webdata\accounts\<site>` (`instagram`, `facebook`; hàm `webdata_dir_for(site)` trong module `accounts`), tách khỏi webview giao diện chính và khỏi trang còn lại, nên đăng xuất một trang không làm mất phiên của trang kia. Kiểu B sẽ dùng lại đúng thư mục của trang đó để giữ đăng nhập.

TikTok và YouTube không cần đăng nhập ở bản đầu **[Claude]**. Google chặn đăng nhập trong WebView nhúng, nên không dùng được cách đăng nhập trong app cho YouTube. Khi YouTube đòi "xác nhận không phải bot", app báo lỗi rõ ràng: "YouTube đang yêu cầu đăng nhập, thử lại sau hoặc dùng link khác". Bản đầu không có cách vượt qua.

**Mức hỗ trợ tối thiểu cho từng nguồn để bản đầu được coi là xong** **[Claude]**:
- TikTok: video (MP4) + MP3 + dòng "Nhạc gốc" khi có. Slideshow ảnh nếu gallery-dl đọc được.
- Instagram: ảnh (gồm carousel), video/reel, MP3 từ video.
- YouTube: video công khai (MP4 các mức chất lượng) + MP3.
- Facebook: video/reel (MP4) + MP3. **Ảnh Facebook chỉ là best-effort** qua gallery-dl, không chặn việc phát hành nếu chưa chạy.

## 8. Cấu trúc để sau này gắn kiểu B

Kiểu B (lướt IG/FB trong app, bấm nút tải ngay trên bài) sẽ tái sử dụng phần lõi của bản A. Vì vậy bản A phải tách phần lõi như sau **[Claude]**:

- Một lệnh lõi trong Rust: `resolve(url) -> MediaInfo` (metadata + danh sách các lựa chọn, mỗi lựa chọn có id, loại, nhãn, link gốc). Lệnh này không biết gì về giao diện.
- Một lệnh lõi: `download(url, option_id, dest_path) -> stream tiến trình`.
- Một module `accounts` quản lý cửa sổ đăng nhập, các thư mục dữ liệu WebView theo từng trang và các file cookie.
- UI kết quả (khung kết quả ở §4.2) là một component độc lập, nhận `MediaInfo`. Kiểu B sẽ hiện chính component này trong một popup.

Chi tiết kiểu B nằm ở `2026-10-25-lumi-browse-mode-design.vi.md`.

## 9. Đóng gói

- Bộ cài NSIS cho Windows x64 (`tauri build`). **[Claude]**
- Mục tiêu dung lượng bộ cài: dưới **10MB** **[Claude]** (vỏ Tauri + frontend). WebView2 đã có sẵn trên Windows 11. Trên Windows 10 dùng bootstrapper của WebView2.
- *[Claude]* Không ký số (code signing) ở bản đầu, vì là app cá nhân. SmartScreen sẽ cảnh báo lần đầu cài, chấp nhận được.

## 10. Phần để người làm tự quyết

- Cấu trúc thư mục dự án, tên module, cách parse output yt-dlp (`-J` cho metadata, `--newline --progress-template` cho tiến trình là gợi ý).
- Cách gộp format thành các mức chất lượng dễ hiểu.
- Hiệu ứng chuyển động, chi tiết khoảng cách và typography, miễn đúng tinh thần kính mờ ở §4.6.
- Cách tải ảnh thu nhỏ (thumbnail CDN có thể đòi referer, có thể phải tải qua Rust rồi đưa về dạng data URL).
- Cách test: phần lõi Rust (phân loại lỗi, chuyển cookie sang định dạng Netscape, gộp format) nên có unit test. Phần gọi mạng thật có thể test thủ công với link mẫu.

## 11. Rủi ro đã biết

- Các trang đổi cấu trúc liên tục. yt-dlp/gallery-dl sẽ có lúc hỏng cho đến khi có bản cập nhật. Đây là lý do có nút Cập nhật và thông báo lỗi tương ứng.
- Dùng cookie tài khoản thật để tải tự động có thể khiến Instagram/Facebook gắn cờ tài khoản nếu dùng dày đặc. Với tần suất dùng cá nhân thì rủi ro thấp.
- Link gốc hết hạn và không mở được ngoài app (đã ghi trong UI, §4.3).
- Việc tải nội dung phải tuân theo điều khoản của từng nền tảng và bản quyền của người đăng. App phục vụ mục đích cá nhân.
