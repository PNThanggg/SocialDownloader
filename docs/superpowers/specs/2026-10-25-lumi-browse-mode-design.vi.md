# Lumi — Tài liệu nâng cấp: chế độ lướt trong app (kiểu B)

Ngày: 2026-10-25 · Trạng thái: chuẩn bị sẵn, **chưa làm**. Chỉ làm khi người dùng yêu cầu chuyển sang kiểu B.

> **Ngôn ngữ:** Tiếng Việt — bản để đọc. Claude triển khai theo bản tiếng Anh `2026-10-25-lumi-browse-mode-design.md`. Nếu hai bản khác nhau thì bản tiếng Anh được tính. Khi sửa thì sửa cả hai.

Đọc trước: `2026-10-25-lumi-design.vi.md` (bản A). Tài liệu này giả định bản A đã chạy được và có phần lõi tách riêng như §8 của bản A.

Ký hiệu như bản A: **[Người dùng]** = người dùng đã quyết, **[Claude]** = quyết định của người viết, có thể đổi.

## 1. Mục đích

Người dùng hay lướt Instagram và Facebook. Họ nói sẵn sàng "lướt fb với ig trên app luôn" **[Người dùng]**. Kiểu B cho phép mở IG/FB ngay trong Lumi: thấy bài nào muốn lưu thì bấm một nút, khỏi phải copy link rồi dán. Kiểu A (dán link) vẫn giữ nguyên **[Claude]**.

Người dùng chọn làm kiểu A trước, kèm tài liệu này để sau này chuyển sang thì triển khai được ngay **[Người dùng]**.

## 2. Mục tiêu / ngoài phạm vi

**Mục tiêu**
- Mở Instagram hoặc Facebook trong một cửa sổ Lumi, giữ sẵn đăng nhập đã kết nối ở bản A.
- Trên mỗi bài (hoặc ít nhất trên bài đang mở/đang xem), có một nút **Tải** nổi.
- Bấm vào thì hiện đúng khung kết quả của bản A (MP3/MP4/ảnh, Copy/Tải) cho bài đó.

**Ngoài phạm vi** **[Claude]**
- Đọc trực tiếp file media từ trang (video IG/FB phát qua `blob:`/MSE nên không dùng được). Luôn lấy **URL bài viết** rồi đưa qua `resolve(url)` của bản A.
- TikTok/YouTube trong app (người dùng chỉ nhắc FB và IG).
- Chặn quảng cáo hay chỉnh sửa giao diện IG/FB ngoài nút Tải.

## 3. Thay đổi trên giao diện chính

- Thêm hai nút nhỏ cạnh bánh răng ở bố cục A: **Lướt Instagram** và **Lướt Facebook** **[Claude]**. Bố cục A vẫn là màn hình chính **[Người dùng]** đã chọn.
- Nếu chưa kết nối tài khoản thì bấm vào sẽ mở luồng đăng nhập của bản A trước.

## 4. Cửa sổ lướt

- Một `WebviewWindow` Tauri mới, tải `https://www.instagram.com/` hoặc `https://www.facebook.com/`.
- Dùng **chung thư mục dữ liệu WebView** với cửa sổ đăng nhập ở bản A (§7), nên không phải đăng nhập lại **[Claude]**.
- Có thanh trên mỏng theo phong cách kính mờ, gồm: Back / Forward / Reload, ô hiển thị URL (chỉ đọc), nút **Tải bài đang mở** (phương án dự phòng ở §5.3).
  - *Gợi ý cách làm:* thanh này là một webview giao diện Lumi riêng. Nếu muốn ghép chung cửa sổ thì dùng tính năng multi-webview của Tauri (đang `unstable`). Cách đơn giản hơn là thanh công cụ nằm trong cửa sổ riêng gắn liền phía trên. Người làm tự chọn.
- Chỉ cho điều hướng trong các domain `instagram.com`, `facebook.com`, `fbcdn.net`, `cdninstagram.com` và các domain đăng nhập/2FA liên quan. Link ra ngoài thì mở bằng trình duyệt mặc định **[Claude]**.

## 5. Nút Tải trên bài

### 5.1 Script chèn vào trang
- Dùng `initialization_script` của Tauri để chèn một script vào mọi trang IG/FB.
- IG/FB là SPA, nên script dùng `MutationObserver` để bắt bài mới khi cuộn và khi điều hướng nội bộ.
- Với mỗi bài phát hiện được, gắn một nút tròn nhỏ nổi ở góc bài, đặt trong Shadow DOM riêng để CSS của trang không đè lên.
- *[Claude]* Có thể kèm `IntersectionObserver` để chỉ hiện nút trên bài đang chiếm phần lớn màn hình, đỡ rối mắt.

### 5.2 Nhận diện bài và lấy URL
Mỗi bài phải quy về một **URL bài viết chuẩn** để đưa vào `resolve(url)`.

- **Instagram** (dễ): mỗi bài là một `article` hoặc khung reel. Trong đó có link dạng `/p/<shortcode>/` hoặc `/reel/<shortcode>/`, hoặc story `/stories/<user>/<id>/`. URL chuẩn: `https://www.instagram.com/p/<shortcode>/`.
- **Facebook** (khó, DOM bị làm rối, đổi liên tục): tìm trong khung bài các link có mẫu `/reel/<id>`, `/videos/<id>`, `/watch/?v=<id>`, `/posts/<id>`, `/photo/?fbid=`, `permalink.php?story_fbid=`, `/share/`. Thường là link ở timestamp của bài. Không tìm thấy thì không gắn nút cho bài đó.
- Các selector và mẫu link cần để trong **một file cấu hình riêng** (ví dụ `browse-selectors.json`) để sửa nhanh khi IG/FB đổi DOM, khỏi phải sửa logic **[Claude]**.

### 5.3 Phương án dự phòng: "Tải bài đang mở"
Khi người dùng mở hẳn một bài (URL cửa sổ là URL bài), nút **Tải bài đang mở** trên thanh công cụ lấy luôn URL hiện tại. Cách này vẫn chạy được dù script ở §5.1 hỏng vì IG/FB đổi giao diện. **Nên làm trước** để có kiểu B dùng được sớm, rồi mới làm nút trên từng bài **[Claude]**.

### 5.4 Gửi URL từ trang về Lumi (an toàn)
Không mở IPC của Tauri cho domain bên ngoài (tức không cấp capability `remote` cho instagram.com/facebook.com). Như vậy script của IG/FB không gọi được lệnh Rust nào **[Claude]**.

Cách làm đề xuất:
- Khi bấm nút, script chèn vào sẽ điều hướng tới một URL đặc biệt, ví dụ `https://lumi.invalid/pick?url=<encoded>`.
- Rust chặn điều hướng này bằng handler `on_navigation` của cửa sổ: hủy điều hướng, đọc tham số `url`, **kiểm tra** URL thuộc instagram.com/facebook.com và đúng một trong các mẫu ở §5.2, sau đó mới gọi `resolve(url)`.
- Từ đây, dù trang có tự gọi URL này thì cũng chỉ mở được popup kết quả. Muốn tải vẫn phải bấm trong UI của Lumi và qua hộp thoại Save as.

## 6. Popup kết quả

- Bấm nút → mở một cửa sổ nhỏ kiểu kính mờ, luôn nằm trên, đặt cạnh cửa sổ lướt. Popup hiện **đúng component khung kết quả của bản A** (§4.2 bản A) với `MediaInfo` vừa resolve **[Claude]**.
- Trong lúc resolve thì hiện skeleton. Lỗi hiện theo đúng bảng lỗi ở §6 bản A.
- Tải vẫn đi qua hộp thoại Save as của Windows **[Người dùng]** (quyết định ở bản A).
- Esc hoặc bấm ra ngoài thì đóng popup. Tải đang chạy thì vẫn tiếp tục *[Claude]*.

## 7. Phần lõi tái sử dụng từ bản A (không viết lại)

- `resolve(url) -> MediaInfo`
- `download(url, option_id, dest_path)`
- module `accounts` (thư mục dữ liệu WebView, file cookie mã hóa DPAPI)
- component khung kết quả và bảng lỗi

Cookie: vì cửa sổ lướt dùng chung thư mục dữ liệu WebView, cookie trong lúc lướt có thể được làm mới. **[Claude]** Mỗi lần mở popup thì đọc lại cookie từ WebView và ghi đè file cookie, để yt-dlp/gallery-dl luôn dùng phiên mới nhất.

## 8. Thứ tự triển khai gợi ý

1. Cửa sổ lướt IG + dùng chung đăng nhập + thanh công cụ + **Tải bài đang mở** (§5.3) + popup (§6).
2. Làm tương tự với FB.
3. Script chèn nút trên từng bài cho IG (§5.1–5.2).
4. Script chèn nút trên từng bài cho FB.
5. Chặn domain lạ, mở link ngoài bằng trình duyệt (§4).

Xong bước 1 là đã dùng được, các bước sau làm cho tiện hơn.

## 9. Rủi ro

- **DOM của IG/FB đổi thường xuyên**, nhất là FB. Nút trên bài sẽ có lúc hỏng. Cách giảm thiểu: file selector riêng (§5.2) và nút dự phòng (§5.3).
- **Tài khoản bị gắn cờ:** lướt trong WebView nhúng và chèn script có thể bị IG/FB phát hiện là trình duyệt lạ. Không giả user-agent quá đà, không tự cuộn hay tự bấm thay người dùng. Mọi hành động đều do người dùng bấm.
- **Bảo mật:** không mở IPC cho domain ngoài (§5.4). Không đọc hay lưu nội dung trang ngoài URL bài được chọn.

## 10. Câu hỏi để hỏi người dùng khi bắt đầu làm kiểu B

- Nút tải nên nổi trên từng bài, hay chỉ cần nút "Tải bài đang mở" trên thanh công cụ là đủ?
- Popup kết quả nên là cửa sổ nhỏ riêng hay một ngăn trượt ra bên cạnh trong cửa sổ lướt?
- Có muốn thêm TikTok vào chế độ lướt không?
