import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import "./globals.css";

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  metadataBase: new URL("https://quotabar.fyi"),
  title: "QuotaBar — Bring back Codex’s five-hour limit",
  description:
    "QuotaBar tracks Codex usage and pauses new Mac app prompts when your five-hour budget runs out.",
  icons: {
    icon: "/icon.png",
    shortcut: "/icon.png",
  },
  openGraph: {
    title: "QuotaBar brings back Codex’s five-hour limit.",
    description: "Track your Codex usage and keep one long session from burning through your weekly allowance.",
    url: "https://quotabar.fyi",
    siteName: "QuotaBar",
    images: [{ url: "/og.png", width: 1200, height: 630, alt: "QuotaBar for macOS" }],
    type: "website",
  },
  twitter: {
    card: "summary_large_image",
    title: "QuotaBar brings back Codex’s five-hour limit.",
    description: "Track your Codex usage and keep one long session from burning through your weekly allowance.",
    images: ["/og.png"],
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body
        className={`${geistSans.variable} ${geistMono.variable} antialiased`}
      >
        {children}
      </body>
    </html>
  );
}
