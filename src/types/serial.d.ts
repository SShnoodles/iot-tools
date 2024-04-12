export interface SerialPort {
  serialPort: string;
  baudRate: number;
  autoSend: boolean;
  autoSendTimes: number;
  sendFormat: number;
  sendContent: string;
  returnContent: string;
}

export interface Option {
  label: string;
  value: any;
}