# Lumi — danh sách tự kiểm tra bằng tay

Cài bằng `src-tauri\target\release\bundle\nsis\Lumi_0.1.0_x64-setup.exe` (hoặc chạy `npm run tauri dev`).

1. **Máy sạch:** đóng Lumi, đổi tên `%LOCALAPPDATA%\Lumi` thành `Lumi.bak`, mở app. Màn "Đang chuẩn bị…" tải xong yt-dlp rồi gallery-dl (có thanh tiến trình), sau đó vào màn chính.
2. **Không mạng lúc đầu:** tắt mạng, xoá thư mục Lumi vừa tạo, mở app → "Không có kết nối mạng." + **Thử lại**. Bật mạng, bấm Thử lại → cài tiếp được.
3. **ffmpeg:** dán link TikTok, bấm Tải ở dòng MP3, chọn chỗ lưu. Hộp "Cần tải thêm ffmpeg (~xx MB) một lần." hiện đúng dung lượng thật (~200MB). Bấm Tải → có tiến trình, xong file MP3 về đúng chỗ đã chọn, **không** hỏi Save as lần nữa. Mở file: tag tiêu đề = tiêu đề video.
4. **Nhạc gốc TikTok:** tải dòng "Nhạc gốc" → file MP3 có tên bài + nghệ sĩ.
5. **deno:** link YouTube đầu tiên → hỏi deno. Bấm **Để sau** → vẫn có kết quả + "Chưa có đủ chất lượng" + nút "Lấy đủ chất lượng". Link YouTube khác → không hỏi nữa. Bấm "Lấy đủ chất lượng" → cài deno rồi đọc lại link.
6. **Facebook:** dán link một reel Facebook công khai, tải dòng MP4 → file chạy được.
7. **Playlist:** dán `https://www.youtube.com/playlist?list=PL59FEE129ADFF2B12` → vài giây sau báo "Lumi chưa hỗ trợ tải cả playlist hay kênh…", app không bị treo.
8. **Hủy:** bắt đầu tải một video dài rồi bấm Hủy → không còn file `.part` trong thư mục đã chọn. Thử thêm "Tải tất cả" trên bài nhiều ảnh rồi Hủy.
9. **Settings → Tài khoản:** ⚙ → Instagram → **Kết nối**. Trong lúc cửa sổ đăng nhập mở, dòng đó hiện "Đang chờ đăng nhập…" (kể cả khi đóng rồi mở lại Settings). Đăng nhập xong → "Đã kết nối" + **Đăng xuất**. Đăng xuất Instagram không làm mất kết nối Facebook.
10. **Đăng nhập thật:** sau khi kết nối Instagram, dán link một bài chỉ xem được khi đăng nhập → đọc và tải được. Mở `%LOCALAPPDATA%\Lumi\tmp` → **không** còn file `cookies-*.txt`. Thử nút "Log in with Facebook" trong cửa sổ đăng nhập Instagram, ghi lại popup có mở/đóng bình thường không.
11. **Settings → Công cụ → Cập nhật:** chỉ cập nhật yt-dlp và gallery-dl (không tải lại ffmpeg/deno), xong hiện "Đã cập nhật" và version đúng.
12. **Thiếu yt-dlp khi đang chạy:** xoá `%LOCALAPPDATA%\Lumi\bin\yt-dlp.exe` lúc app mở, dán link → "Thiếu công cụ yt-dlp." + **Cài lại công cụ** → bấm vào dùng lại bình thường.
13. **Ngôn ngữ:** Settings → English / Tiếng Việt / Tự động → chữ đổi ngay, mở lại app vẫn giữ.
14. **Giao diện sáng của Windows:** đổi Windows sang Light mode → nền kính của Lumi vẫn tối, chữ đọc rõ.
15. **Lịch sử tải:** tải MP3 rồi MP4 của cùng một link TikTok → bấm nút đồng hồ trên thanh tiêu đề → chỉ có một dòng, có nhãn MP4 và MP3, ghi "vừa xong"/"x phút trước". Bấm **Mở thư mục** → Explorer chọn đúng file. Bấm **Tải lại** → bảng đóng, link nằm trong ô nhập và được đọc lại, tải được bình thường. Đóng rồi mở lại app → lịch sử vẫn còn. **Xóa hết** phải bấm hai lần.
16. **Một bản duy nhất:** mở Lumi lần hai → chỉ cửa sổ cũ được đưa lên.
17. Khôi phục: xoá thư mục Lumi mới, đổi `Lumi.bak` về `Lumi` nếu muốn giữ dữ liệu cũ.
