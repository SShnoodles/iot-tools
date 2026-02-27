export interface SerialPort {
  serialPort: string;
  baudRate: number;
  autoSend: boolean;
  autoSendTimes: number;
  sendFormat: number;
  sendContent: string;
  receiveFormat: number;
  receiveContent: string;
  showSend: boolean,
  showTime: boolean,
}

export interface Option {
  label: string;
  value: any;
}

export interface SerialPortLog {
  direction: "TX" | "RX";
  content_hex: string;  // HEX format
  content_ascii: string; // ASCII format
  timestamp: string;
}