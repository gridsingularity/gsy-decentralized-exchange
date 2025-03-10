import { Prop, Schema, SchemaFactory } from '@nestjs/mongoose';
import { Document } from 'mongoose';

@Schema({ timestamps: true })
export class User extends Document {
  @Prop({ required: true, unique: true, index: true })
  did: string;

  @Prop({ index: true })
  gsyDexAddress?: string;

  @Prop({ type: Object })
  metadata?: Record<string, any>;

  @Prop({ default: false })
  hasVerifiedCredential: boolean;
}

export const UserSchema = SchemaFactory.createForClass(User);