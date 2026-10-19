#include "afterdusk_raw.h"

#include <cstdio>
#include <cstring>
#include <new>
#include <string>
#include <vector>

#include "libraw/libraw.h"

#ifdef _WIN32
#include <windows.h>
#endif

namespace {

/* Codes d'IFD exposés à Rust (voir afterdusk_raw.h). */
const uint32_t IFD_EXIF = 0;
const uint32_t IFD_TIFF0 = 1;
const uint32_t IFD_GPS = 4;

/* Au-delà, la valeur n'est pas utile en texte (MakerNote, tables). */
const int MAX_VALUES = 16;
const int MAX_ASCII = 256;

struct ExifEntry {
  uint32_t ifd;
  uint32_t tag;
  std::string value;
};

bool has_entry(const std::vector<ExifEntry>& entries, uint32_t ifd, uint32_t tag) {
  for (const ExifEntry& e : entries) {
    if (e.ifd == ifd && e.tag == tag) return true;
  }
  return false;
}

uint16_t read_u16(LibRaw_abstract_datastream* s, unsigned order) {
  unsigned char b[2] = {0, 0};
  s->read(b, 1, 2);
  return order == 0x4949 ? uint16_t(b[0] | (b[1] << 8)) : uint16_t((b[0] << 8) | b[1]);
}

uint32_t read_u32(LibRaw_abstract_datastream* s, unsigned order) {
  unsigned char b[4] = {0, 0, 0, 0};
  s->read(b, 1, 4);
  return order == 0x4949 ? uint32_t(b[0]) | (uint32_t(b[1]) << 8) | (uint32_t(b[2]) << 16) |
                               (uint32_t(b[3]) << 24)
                         : (uint32_t(b[0]) << 24) | (uint32_t(b[1]) << 16) |
                               (uint32_t(b[2]) << 8) | uint32_t(b[3]);
}

/* Met en texte la valeur d'une balise TIFF ; le flux est positionné sur la
 * valeur par LibRaw avant l'appel. Renvoie false si la balise est ignorée. */
bool format_value(LibRaw_abstract_datastream* s, int type, int len, unsigned order,
                  std::string& out) {
  char buf[64];
  if (type == 2) { /* ASCII */
    int n = len < MAX_ASCII ? len : MAX_ASCII;
    std::string text(size_t(n), '\0');
    if (n > 0) s->read(&text[0], 1, size_t(n));
    size_t end = text.find('\0');
    if (end != std::string::npos) text.resize(end);
    while (!text.empty() && text.back() == ' ') text.pop_back();
    out = text;
    return true;
  }
  if (len <= 0 || len > MAX_VALUES) return false;
  for (int i = 0; i < len; i++) {
    if (i > 0) out += ' ';
    switch (type) {
      case 1: /* BYTE */
      case 7: /* UNDEFINED */
        snprintf(buf, sizeof buf, "%u", unsigned(s->get_char() & 0xff));
        break;
      case 6: /* SBYTE */
        snprintf(buf, sizeof buf, "%d", int(static_cast<signed char>(s->get_char())));
        break;
      case 3: /* SHORT */
        snprintf(buf, sizeof buf, "%u", unsigned(read_u16(s, order)));
        break;
      case 8: /* SSHORT */
        snprintf(buf, sizeof buf, "%d", int(int16_t(read_u16(s, order))));
        break;
      case 4: /* LONG */
        snprintf(buf, sizeof buf, "%u", read_u32(s, order));
        break;
      case 9: /* SLONG */
        snprintf(buf, sizeof buf, "%d", int32_t(read_u32(s, order)));
        break;
      case 5: { /* RATIONAL */
        uint32_t num = read_u32(s, order);
        uint32_t den = read_u32(s, order);
        snprintf(buf, sizeof buf, "%u/%u", num, den);
        break;
      }
      case 10: { /* SRATIONAL */
        int32_t num = int32_t(read_u32(s, order));
        int32_t den = int32_t(read_u32(s, order));
        snprintf(buf, sizeof buf, "%d/%d", num, den);
        break;
      }
      case 11: { /* FLOAT */
        uint32_t bits = read_u32(s, order);
        float f;
        std::memcpy(&f, &bits, sizeof f);
        snprintf(buf, sizeof buf, "%g", double(f));
        break;
      }
      default:
        return false;
    }
    out += buf;
  }
  return true;
}

}  // namespace

struct ad_raw {
  LibRaw processor;
  ad_raw_info info;
  std::vector<ExifEntry> exif;
  libraw_processed_image_t* thumb = nullptr;
  libraw_processed_image_t* image = nullptr;
  ad_raw_color color{};
};

namespace {

void exif_callback(void* context, int tag, int type, int len, unsigned int ord, void* ifp,
                   INT64 /*base*/) {
  ad_raw* raw = static_cast<ad_raw*>(context);
  uint32_t flags = uint32_t(tag) & 0xffff0000u;
  uint32_t number = uint32_t(tag) & 0xffffu;
  uint32_t ifd;
  if (flags == 0) {
    ifd = IFD_EXIF;
  } else if (flags == 0x100000u || flags == 0x30000u) {
    ifd = IFD_TIFF0; /* 0x30000 : IFD0 des RW2 Panasonic */
  } else if (flags == 0x50000u) {
    ifd = IFD_GPS;
  } else {
    return;
  }
  /* Certains fichiers contiennent plusieurs IFD EXIF (aperçus) : le premier fait foi. */
  if (has_entry(raw->exif, ifd, number)) return;
  std::string value;
  if (format_value(static_cast<LibRaw_abstract_datastream*>(ifp), type, len, ord, value)) {
    raw->exif.push_back(ExifEntry{ifd, number, value});
  }
}

void copy_str(char* dst, size_t cap, const char* src) {
  std::strncpy(dst, src, cap - 1);
  dst[cap - 1] = '\0';
}

void fill_info(ad_raw* raw) {
  const libraw_data_t& d = raw->processor.imgdata;
  ad_raw_info& i = raw->info;
  std::memset(&i, 0, sizeof i);
  copy_str(i.make, sizeof i.make, d.idata.make);
  copy_str(i.model, sizeof i.model, d.idata.model);
  copy_str(i.normalized_make, sizeof i.normalized_make, d.idata.normalized_make);
  copy_str(i.normalized_model, sizeof i.normalized_model, d.idata.normalized_model);
  copy_str(i.software, sizeof i.software, d.idata.software);
  copy_str(i.lens_make, sizeof i.lens_make, d.lens.LensMake);
  copy_str(i.lens, sizeof i.lens, d.lens.Lens);
  copy_str(i.body_serial, sizeof i.body_serial, d.shootinginfo.BodySerial);
  copy_str(i.artist, sizeof i.artist, d.other.artist);
  i.iso_speed = d.other.iso_speed;
  i.shutter = d.other.shutter;
  i.aperture = d.other.aperture;
  i.focal_len = d.other.focal_len;
  i.focal_len_35mm = d.lens.FocalLengthIn35mmFormat;
  i.timestamp = int64_t(d.other.timestamp);
  i.raw_width = d.sizes.raw_width;
  i.raw_height = d.sizes.raw_height;
  i.width = d.sizes.width;
  i.height = d.sizes.height;
  i.flip = d.sizes.flip;
  i.filters = d.idata.filters;
  i.colors = d.idata.colors;
  i.dng_version = d.idata.dng_version;
  i.gps_parsed = d.other.parsed_gps.gpsparsed;
  for (int k = 0; k < 3; k++) {
    i.gps_latitude[k] = d.other.parsed_gps.latitude[k];
    i.gps_longitude[k] = d.other.parsed_gps.longitude[k];
  }
  i.gps_altitude = d.other.parsed_gps.altitude;
  i.gps_latref = d.other.parsed_gps.latref;
  i.gps_longref = d.other.parsed_gps.longref;
  i.gps_altref = d.other.parsed_gps.altref;
  i.thumb_count = d.thumbs_list.thumbcount;
}

}  // namespace

extern "C" {

ad_raw* ad_raw_open(const char* utf8_path, int* error) {
  if (error) *error = 0;
  ad_raw* raw = new (std::nothrow) ad_raw();
  if (!raw) {
    if (error) *error = LIBRAW_UNSUFFICIENT_MEMORY;
    return nullptr;
  }
  raw->processor.set_exifparser_handler(exif_callback, raw);

  int ret;
#ifdef _WIN32
  /* Chemins Unicode : LibRaw attend du wchar_t sous Windows. */
  int wlen = MultiByteToWideChar(CP_UTF8, 0, utf8_path, -1, nullptr, 0);
  if (wlen <= 0) {
    ret = LIBRAW_IO_ERROR;
  } else {
    std::wstring wpath(size_t(wlen), L'\0');
    MultiByteToWideChar(CP_UTF8, 0, utf8_path, -1, &wpath[0], wlen);
    ret = raw->processor.open_file(wpath.c_str());
  }
#else
  ret = raw->processor.open_file(utf8_path);
#endif

  if (ret != LIBRAW_SUCCESS) {
    if (error) *error = ret;
    delete raw;
    return nullptr;
  }
  fill_info(raw);
  return raw;
}

void ad_raw_close(ad_raw* raw) {
  if (!raw) return;
  if (raw->thumb) LibRaw::dcraw_clear_mem(raw->thumb);
  if (raw->image) LibRaw::dcraw_clear_mem(raw->image);
  raw->processor.recycle();
  delete raw;
}

const ad_raw_info* ad_raw_get_info(const ad_raw* raw) { return raw ? &raw->info : nullptr; }

size_t ad_raw_exif_count(const ad_raw* raw) { return raw ? raw->exif.size() : 0; }

int ad_raw_exif_get(const ad_raw* raw, size_t index, uint32_t* ifd, uint32_t* tag,
                    const char** value) {
  if (!raw || index >= raw->exif.size()) return 0;
  const ExifEntry& e = raw->exif[index];
  *ifd = e.ifd;
  *tag = e.tag;
  *value = e.value.c_str();
  return 1;
}

int ad_raw_thumb_count(const ad_raw* raw) {
  if (!raw) return 0;
  int count = raw->processor.imgdata.thumbs_list.thumbcount;
  return count < LIBRAW_THUMBNAIL_MAXCOUNT ? count : LIBRAW_THUMBNAIL_MAXCOUNT;
}

int ad_raw_thumb_item(const ad_raw* raw, int index, int* format, uint16_t* width,
                      uint16_t* height, uint32_t* length) {
  if (!raw || index < 0 || index >= ad_raw_thumb_count(raw)) return 0;
  const libraw_thumbnail_item_t& t = raw->processor.imgdata.thumbs_list.thumblist[index];
  *format = int(t.tformat);
  *width = t.twidth;
  *height = t.theight;
  *length = t.tlength;
  return 1;
}

int ad_raw_thumbnail(ad_raw* raw, int index, const uint8_t** data, size_t* size, int* format,
                     uint16_t* width, uint16_t* height, uint16_t* colors, uint16_t* bits) {
  if (!raw) return LIBRAW_UNSPECIFIED_ERROR;
  if (raw->thumb) {
    LibRaw::dcraw_clear_mem(raw->thumb);
    raw->thumb = nullptr;
  }

  int ret = raw->processor.unpack_thumb_ex(index);
  if (ret != LIBRAW_SUCCESS) return ret;

  int err = 0;
  raw->thumb = raw->processor.dcraw_make_mem_thumb(&err);
  if (!raw->thumb) return err ? err : LIBRAW_UNSPECIFIED_ERROR;

  *data = raw->thumb->data;
  *size = raw->thumb->data_size;
  *format = int(raw->thumb->type);
  /* LibRaw ne renseigne pas les dimensions des JPEG : on reprend celles déclarées. */
  const libraw_thumbnail_t& t = raw->processor.imgdata.thumbnail;
  *width = raw->thumb->width ? raw->thumb->width : t.twidth;
  *height = raw->thumb->height ? raw->thumb->height : t.theight;
  *colors = raw->thumb->colors;
  *bits = raw->thumb->bits;
  return LIBRAW_SUCCESS;
}

int ad_raw_develop(ad_raw* raw, int half_size, const uint16_t** data, uint16_t* width,
                   uint16_t* height) {
  return ad_raw_develop_ex(raw, half_size, 0, data, width, height);
}

int ad_raw_develop_ex(ad_raw* raw, int half_size, int fast_xtrans, const uint16_t** data,
                      uint16_t* width, uint16_t* height) {
  if (!raw) return LIBRAW_UNSPECIFIED_ERROR;
  if (raw->image) {
    LibRaw::dcraw_clear_mem(raw->image);
    raw->image = nullptr;
  }
  libraw_output_params_t& p = raw->processor.imgdata.params;
  p.output_color = 5; /* XYZ */
  p.gamm[0] = 1.0;    /* linéaire */
  p.gamm[1] = 1.0;
  p.no_auto_bright = 1;
  p.use_camera_wb = 1;
  p.use_camera_matrix = 1;
  p.output_bps = 16;
  p.half_size = half_size ? 1 : 0;
  /* AHD ; le X-Trans garde son propre dématriçage (trois passes au-delà de 2). */
  p.user_qual = (fast_xtrans && raw->processor.imgdata.idata.filters == 9) ? 2 : 3;
  p.highlight = 0; /* hautes lumières écrêtées : la reconstruction viendra avec les réglages */

  int ret = raw->processor.unpack();
  if (ret != LIBRAW_SUCCESS) return ret;
  /* dcraw_process remplace pre_mul par cam_mul : relevé avant. */
  const libraw_colordata_t& c = raw->processor.imgdata.color;
  for (int i = 0; i < 4; i++) {
    raw->color.cam_mul[i] = c.cam_mul[i];
    raw->color.pre_mul[i] = c.pre_mul[i];
    for (int j = 0; j < 3; j++) raw->color.rgb_cam[j][i] = c.rgb_cam[j][i];
  }
  raw->color.colors = raw->processor.imgdata.idata.colors;
  ret = raw->processor.dcraw_process();
  if (ret != LIBRAW_SUCCESS) return ret;
  int err = 0;
  raw->image = raw->processor.dcraw_make_mem_image(&err);
  if (!raw->image) return err ? err : LIBRAW_UNSPECIFIED_ERROR;
  if (raw->image->type != LIBRAW_IMAGE_BITMAP || raw->image->colors != 3 ||
      raw->image->bits != 16) {
    return LIBRAW_UNSPECIFIED_ERROR;
  }
  *data = reinterpret_cast<const uint16_t*>(raw->image->data);
  *width = raw->image->width;
  *height = raw->image->height;
  return LIBRAW_SUCCESS;
}

const ad_raw_color* ad_raw_get_color(const ad_raw* raw) { return raw ? &raw->color : nullptr; }

const char* ad_raw_strerror(int error) { return libraw_strerror(error); }

const char* ad_raw_version(void) { return LibRaw::version(); }

}  // extern "C"
