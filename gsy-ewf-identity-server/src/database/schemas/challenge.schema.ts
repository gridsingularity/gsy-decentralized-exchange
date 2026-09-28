import { Prop, Schema, SchemaFactory } from '@nestjs/mongoose';
import { Document } from 'mongoose';

@Schema()
export class Challenge extends Document {
  @Prop({ required: true, unique: true })
  id: string;

  @Prop({ required: true })
  challenge: string;

  @Prop({ required: true })
  did: string;

  @Prop()
  timestamp: Date;

  @Prop({ default: false })
  used: boolean;

  @Prop({ expires: '10m', default: Date.now })
  createdAt: Date;
}

export const ChallengeSchema = SchemaFactory.createForClass(Challenge);