/**
 * The Strata backdrop: four drifting radial blobs plus two diagonal streak
 * layers, all blurred behind the glass window surface. Pure decoration —
 * pointer-events pass straight through.
 */
export default function Wallpaper() {
  return (
    <>
      <div
        className="pointer-events-none absolute"
        style={{ inset: "-14%", filter: "blur(70px)", opacity: 0.95 }}
      >
        <div
          className="drift-a absolute rounded-full"
          style={{
            left: "-6%",
            top: "-12%",
            width: "78%",
            height: "86%",
            background: "radial-gradient(circle at 40% 40%, var(--w2), transparent 66%)",
          }}
        />
        <div
          className="drift-b absolute rounded-full"
          style={{
            right: "-14%",
            top: "2%",
            width: "70%",
            height: "78%",
            background: "radial-gradient(circle at 60% 45%, var(--w3), transparent 64%)",
          }}
        />
        <div
          className="drift-c absolute rounded-full"
          style={{
            left: "14%",
            bottom: "-24%",
            width: "82%",
            height: "72%",
            background: "radial-gradient(circle at 50% 50%, var(--w4), transparent 62%)",
          }}
        />
        <div
          className="drift-d absolute rounded-full"
          style={{
            right: "6%",
            bottom: "-10%",
            width: "56%",
            height: "60%",
            background: "radial-gradient(circle at 50% 50%, var(--w5), transparent 66%)",
          }}
        />
      </div>
      <div
        className="pointer-events-none absolute inset-0 opacity-50"
        style={{
          background:
            "repeating-linear-gradient(118deg, rgba(255,255,255,.055) 0 1px, transparent 1px 5px), repeating-linear-gradient(118deg, rgba(255,255,255,.10) 0 22px, transparent 22px 90px)",
        }}
      />
      <div
        className="pointer-events-none absolute inset-0"
        style={{
          background:
            "linear-gradient(118deg, transparent 34%, rgba(255,255,255,.16) 46%, rgba(255,255,255,.03) 50%, transparent 62%)",
        }}
      />
    </>
  );
}
